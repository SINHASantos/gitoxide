#!/bin/bash
set -eu -o pipefail

# list of urls that should be tested for all platforms
tests=()
# urls only intended for testing on Unix platforms
tests_unix=()
# urls only intended for testing on Windows
tests_windows=()

# The contents and structure of this loop are a adaption
# from git's own test suite (t/t5500-fetch-pack.sh).
# Please do not change this loop and instead add additional
# test cases at the bottom of this file.
for path in "repo" "re:po" "re/po"; do
  for protocol in "ssh+git" "git+ssh" "git" "ssh"; do
    for host in "host" "user@host" "user@[::1]" "user@::1"; do
      for port_separator in "" ":"; do
        tests+=("$protocol://$host$port_separator/$path")

        tests+=("$protocol://$host$port_separator/~$path")
      done
    done
    for host in "host" "User@host" "User@[::1]"; do
      tests+=("$protocol://$host:22/$path")
    done
  done
  for protocol in "file"; do
    tests_unix+=("$protocol://$host/$path")

    tests_windows+=("$protocol://$host/$path")
    tests_windows+=("$protocol:///$path")

    tests_unix+=("$protocol://$host/~$path")
    tests_windows+=("$protocol://$host/~$path")
  done
  for host in "nohost" "nohost:12" "[::1]" "[::1]:23" "[" "[:aa"; do
    tests+=("./$host:$path")
    tests+=("./$protocol:$host/~$path")
  done
  protocol="ssh"
  for host in "host" "[::1]"; do
    tests+=("$host:$path")

    tests+=("$host:/~$path")
  done
done

# These two test cases are from git's test suite as well.
tests_windows+=("file://c:/repo")
tests_windows+=("c:repo")

tests_unix+=("${tests[@]}")
tests_windows+=("${tests[@]}")

for url in "${tests[@]}"
do
  echo ";" # there are no `;` in the tested urls
  git fetch-pack --diag-url "$url"
done >git-baseline.generic

# TODO: testing of platform specific behavior

