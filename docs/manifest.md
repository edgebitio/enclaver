---
title: "Manifest File"
layout: docs-enclaver-single
category: reference
weight: 15
---

# Enclaver Manifest File

Enclaver relies on a manifest file to understand how to transform your container into an enclave and run it securely. Egress and Ingress rules are encoded into the image to ensure portability with the [enclave image][format], which enhances security.

The file is YAML formatted and passed to Enclaver via the `-f` flag. By default, Enclaver looks for `enclaver.yaml` in the current directory.

```sh
$ enclaver build -f enclaver.yaml
```

## Example Manifest

```yaml
version: v1
name: "example-enclave"
target: "testapp:enclave-latest"
sources:
  app: "testapp:latest"
defaults:
  memory_mb: 4096
egress:
  allow:
    - google.com
    - www.google.com
ingress:
  - listen_port: 8080
```

An enclave is not required to have both ingress or egress, but without one of these it is not very useful. All egress locations, including internal VPC addresses or hostnames for AWS services must be declared.

Enclaver uses an HTTP/HTTPS proxy for enforcement and the usual `http_proxy`, `https_proxy` and `no_proxy` environment variables are set correctly.

In the future, a more transparent TCP proxy mode will be added to ease integration with applications. See [Issue #69](https://github.com/edgebitio/enclaver/issues/69) for more details.

## Manifest Specification

- **version** (string): Required. Used to differentiate different specification versions. Only `v1` exists right now.
- **name** (string): Required. Informational name used to organize different manifests.
- **target** (string): Required. Name and tag of the Docker container outputted from the build process. Any valid Docker strings are acceptible, including custom registries and hostnames.
- **sources** (object): Required. Information about input container(s) to the build process
  - **app**: (string): Required. Name and tag of the Docker container that contains your application code. Any valid Docker strings are acceptible, including custom registries and hostnames.
- **defaults** (object): Default resource requirements for running the application. Requirements may be overridden at runtime.
  - **cpu_count** (integer): Number of CPUs dedicated to the enclave. Defaults to 2 if not specified here.
  - **memory_mb** (integer): Megabytes of memory dedicated to the enclave. Defaults to 4096 if not specified here.
- **egress** (object): Information about egress traffic leaving the enclave. The policy is deny by default and supports `*` single wildcards for matching a specific position of a subdomain (`web.*.example.com`) or `**` greedy wildcards that match all (`**.example.com`).
  - **allow**: (list of strings): List of allowed hostnames, IP addresses, or CIDR ranges that traffic may flow out of the enclave to. The enforcement is strict, so any redirects must list _all_ of the encountered addresses.
  - **deny**: (list of strings): List of denied hostnames, IP addresses, or CIDR ranges that traffic may _not_ flow out of the enclave to. Deny rules take precedence over allow rules.
- **ingress** (list of objects): Information about ingress traffic entering the enclave. Applications can listen on multiple ports.
  - **listen_port** (integer): Required. Valid port number for the proxy to listen for traffic on.

[format]: architecture.md#enclaver-image-format
