---
title: "Enclaver Commands"
layout: docs-enclaver-single
category: reference
weight: 10
---

# Enclaver Commands

Enclaver is shipped as a single `enclaver` binary that fulfills two main use-cases:

1. Package an exising Docker image into a self-executing Enclaver container image for distribution
1. Easily run a packaged Enclaver container image for testing - without typing long Docker commands

In production, `enclaver build` should be used in a CI workflow, and the container images that it creates
can be distributed and run using existing container registries, Docker, Kubernetes, etc.

## Build

```sh
$ enclaver build [options]
```

Builds an OCI container image in [Enclaver image format][format] containing the components that [run outside][outside] and [inside the enclave][inside]. Once built, the container is named after the `target` field of your [enclave manifest file][manifest].

| Flag | Type | Description |
|:-----|:-----|:------------|
| `-f`, `--file` | String (Default=enclaver.yaml) | Path on disk to your enclave manifest file. |
| `--eif-only` | String | If set, build only the components that run inside of the enclave. EIF is written to the provided path on disk and the containing directory must exist. |
| `--pull` | Boolean (Default=false) | Force a pull of source images. By default, if a local image matching a specified source is found, it will be used without pulling. |

## Run

```sh
$ enclaver run [OPTIONS] [image]
```

Run a packaged Enclaver container image without typing long Docker commands.

This command is a convenience utility that runs a pre-existing Enclaver image in the local Docker
Daemon. It is equivalent to running the image with Docker, and passing:

```sh
    --device=/dev/nitro_enclaves:/dev/nitro_enclaves:rwm
```

Requires a local Docker Daemon to be running, and that this computer is an AWS instance configured
to support Nitro Enclaves.

| Flag | Type | Description |
|:-----|:-----|:------------|
| `-f`, `--file` | String | Enclaver Manifest file in which to look for an image name.<br>Defaults to `enclaver.yaml` if not set and no image is specified. To run a specific image instead, pass the name of the image as an argument. |
| `-p`, `--publish` | String | Port to expose on the host machine, for example: 8080:80 |

[format]: architecture.md#enclaver-image-format
[outside]: architecture.md#components-outside-the-enclave
[inside]: architecture.md#components-inside-the-enclave
[manifest]: manifest.md
