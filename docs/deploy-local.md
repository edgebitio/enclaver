---
title: "Developing Locally"
layout: docs-enclaver-single
category: getting_started
weight: 30
---

## Running an Enclave Locally

An app running with Enclaver starts as a source container image. In most cases, local development of your app can be done by running this container normally with Docker or another runtime.

For testing the specific differences laid out below, see [Deploying on AWS][aws].

### Differences Between Container and Enclave

#### Entrypoint

During `enclaver build`, your original `ENTRYPOINT` in the source container is swapped for Enclaver's PID1, which will still execute your original entrypoint, just not as PID1. While this is a behavior change, it is designed to be non-disruptive in almost all cases. You should not need to test this regularly, and if you do, it is best done within a real enclave instead of locally.

The [Enclaver architecture][arch] document covers this in more detail, specifically the [image format][image] section.

#### Ingress/Egress & Network Policy

Your app running within an enclave will be subject to network policy enforced by the Enclaver network proxies. Running your container locally will not enforce your policy. Today there is not a mechanism to run the proxies locally in a configuration similar to how the Enclave is protected.

#### Communicating with AWS KMS

Enclaver automatically wraps certain calls to KMS with the cryptographic attestation of the enclave, so that key access policies can target those values. When running your code locally, you will not be able to test an access policy that is extremely strict. See more in the [inside proxy][inside-proxy] architecture.

[aws]: deploy-aws.md
[arch]: architecture.md
[image]: architecture.md#enclaver-image-format
[inside-proxy]: architecture.md#inside-proxy