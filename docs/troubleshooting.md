---
title: "Troubleshooting Guide"
layout: docs-enclaver-single
category: troubleshoot
weight: 30
---

# Troubleshooting Nitro Enclaves

## Enable Debug Logging Inside the Enclave

By default, minimal logs are returned from the enclave, as a security precaution. The `--debug-mode` flag will enable debug mode on the enclave, and translate `/dev/console` output to log lines.

```console
$ enclaver run --debug-mode
```

Turning on this flag will change the enclave's attestation document by setting all PCR values to zeros. This may prevent your access to KMS keys or cause other processes to fail if they only trust a specific attestation.

## Setting the Correct Number of x86 vCPUs

Enclaves running on x86 instances must have whole numbers of vCPUs, in multiples of 2, since whole cores (not hyperthreads) are sliced off and dedicated to the enclave, for security.

The minimum core count is 2. The following error appears to be about memory, but is actually due to 1 core being specified instead of 2.

```console
$ enclaver run ...
error: nitro-cli failed: Start allocating memory...
```

In the error log, you will see:

```
  Action: Run Enclave
  Subactions:
    ...
    Start enclave ioctl failed
    The enclave cannot start because full CPU cores have not been set
```

## Building from Large Application Source Containers

Building enclave images from source containers larger than 1GB will sometimes fail. See [linuxkit/linuxkit #3759](https://github.com/linuxkit/linuxkit/issues/3759) for more details on how Nitro CLI uses linuxkit.

```
Linuxkit reported an error while creating the customer ramfs: ...
```

The best workaround for this is to slim down your base image. For example, use `FROM python:3.8-slim` instead of `FROM python:3.8`.
