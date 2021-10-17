use std::collections::BTreeSet;

use cargo_metadata::{DependencyKind, Metadata, Package, PackageId};

use crate::{
    git,
    utils::{is_pre_release_version, package_by_name, workspace_package_by_name},
};

pub mod dependency {
    #[derive(Copy, Clone)]
    pub enum Kind {
        ToBePublished,
        Skipped,
    }
}

pub struct Dependency<'meta> {
    pub package: &'meta Package,
    pub kind: dependency::Kind,
}

pub fn dependencies(
    ctx: &crate::Context,
    verbose: bool,
    add_production_crates: bool,
) -> anyhow::Result<Vec<Dependency<'_>>> {
    let mut seen = BTreeSet::new();
    let mut crates = Vec::new();
    for crate_name in &ctx.crate_names {
        let package = package_by_name(&ctx.meta, crate_name)?;
        if seen.contains(&&package.id) {
            continue;
        }
        if dependency_tree_has_link_to_existing_crate_names(&ctx.meta, package, &crates)? {
            // redo all work which includes the previous tree. Could be more efficient but that would be more complicated.
            seen.clear();
            crates.clear();
        }
        let num_crates_for_publishing_without_dependencies = crates.len();
        let current_skipped =
            depth_first_traversal(ctx, add_production_crates, &mut seen, &mut crates, package, verbose)?;
        if !verbose && !current_skipped.is_empty() {
            log::info!(
                "Skipped {} dependent crates as they didn't change since their last release. Use --verbose/-v to see much more.",
                current_skipped.len()
            );
        }
        crates.extend(current_skipped.into_iter().map(|p| Dependency {
            package: p,
            kind: dependency::Kind::Skipped,
        }));
        if num_crates_for_publishing_without_dependencies == crates.len()
            && !git::has_changed_since_last_release(package, ctx, verbose)?
        {
            log::info!(
                "Skipping provided {} v{} hasn't changed since last released",
                package.name,
                package.version
            );
            continue;
        }
        crates.push(Dependency {
            package,
            kind: dependency::Kind::ToBePublished,
        });
        seen.insert(&package.id);
    }
    Ok(crates)
}

fn depth_first_traversal<'meta>(
    ctx: &'meta crate::Context,
    add_production_crates: bool,
    seen: &mut BTreeSet<&'meta PackageId>,
    crates: &mut Vec<Dependency<'meta>>,
    root: &Package,
    verbose: bool,
) -> anyhow::Result<Vec<&'meta Package>> {
    let mut skipped = Vec::new();
    for dependency in root.dependencies.iter().filter(|d| d.kind == DependencyKind::Normal) {
        let workspace_dependency = match workspace_package_by_name(&ctx.meta, &dependency.name) {
            Some(p) => p,
            None => continue,
        };
        if seen.contains(&&workspace_dependency.id) {
            continue;
        }
        seen.insert(&workspace_dependency.id);
        skipped.extend(depth_first_traversal(
            ctx,
            add_production_crates,
            seen,
            crates,
            workspace_dependency,
            verbose,
        )?);
        if git::has_changed_since_last_release(workspace_dependency, ctx, verbose)? {
            if is_pre_release_version(&workspace_dependency.version) || add_production_crates {
                if verbose {
                    log::info!(
                        "Adding '{}' v{} to set of published crates as it changed since last release",
                        workspace_dependency.name,
                        workspace_dependency.version
                    );
                }
                crates.push(Dependency {
                    package: workspace_dependency,
                    kind: dependency::Kind::ToBePublished,
                });
            } else {
                log::warn!(
                    "Production crate '{}' v{} changed since last release - consider releasing it beforehand.",
                    workspace_dependency.name,
                    workspace_dependency.version
                );
            }
        } else {
            if verbose {
                log::info!(
                    "'{}' v{}  - skipped release as it didn't change",
                    workspace_dependency.name,
                    workspace_dependency.version
                );
            }
            skipped.push(workspace_dependency);
        }
    }
    Ok(skipped)
}

fn dependency_tree_has_link_to_existing_crate_names(
    meta: &Metadata,
    root: &Package,
    existing: &[Dependency<'_>],
) -> anyhow::Result<bool> {
    let mut dependencies = vec![root];
    let mut seen = BTreeSet::new();
    while let Some(package) = dependencies.pop() {
        if !seen.insert(&package.id) {
            continue;
        }
        if existing.iter().any(|d| d.package.id == package.id) {
            return Ok(true);
        }
        dependencies.extend(
            package
                .dependencies
                .iter()
                .filter_map(|dep| workspace_package_by_name(meta, &dep.name)),
        )
    }
    Ok(false)
}
