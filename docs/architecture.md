---
title: "Architecture"
layout: docs-enclaver-single
category: getting_started
weight: 10
---

# Enclaver Architecture

Enclaver consists of a CLI tool for building and running secure enclaves. This document describes the architecture of [the CLI][cli], the container-based [image format][format], and the components that [run outside][outside] and [inside the enclave][inside] to allow your code to make the best use of the enclave's security properties.

Different enclave technologies vary in capabilities and deployment patterns. Enclaver currently only supports AWS Nitro Enclaves and this document reflects this deployment pattern. Support for other cloud provider offerings and Intel/AMD enclave features will come in the future.

![Enclaver Architecture Diagram](img/diagram-enclaver-components.svg)

## Enclaver CLI

Enclaver's CLI is designed for two main use-cases:

1. Build enclave images, sign them and calculate attestations locally on a developer's machine
1. Bootstrap and run the Nitro enclave on an EC2 machine

These use-cases directly map to `enclaver` commands. Refer to the [full list of commands][cmd] to learn about all of the features.

### Building an Enclave

`enclaver build` takes an existing container image of your application code and builds it into a new container image. `--policy-file` specifies a policy for network ingress/egress and runtime parameters of the enclave itself. This policy is packaged into the image because it is distributed with the image and included in its [cryptographic attestation][attestation].

```sh
$ enclaver build registry.example.com/my-app:v1.0 --policy-file policy.yaml
TODO: add output
```

The new image contains both [the outside][outside] and [inside components][inside] of a secure enclave. This artifact is designed to be handled like a regular container. It can be stored in any container registry and be signed with cosign. Read more about the [image format][format] below.

Enclaver will automatically cross-compile to the target architecture of your provided container, which is useful for building on an ARM laptop but running an x86 enclave.

Refer to the [full list of commands][cmd-build] to learn about all of the features.

### Signing an Enclave image

TODO: expand signing instructions

### Running an Enclave

`enclaver run` executes on an EC2 machine to fetch, unpack and run your enclave image. Docker must be installed on the EC2 machine to pull the container image.

After the image is fetched, it is broken apart into [the outside][outside] and [inside components][inside]. The outer components are started first, then the enclave, with the inner components inside, is started.

`enclaver run --verify-before-run attestation.json` will verify an attestation of an image after fetching it, but before executing it. If the comparison fails, the violating PCRs will be logged and the command will fail with an exit code.

`enclaver run --debug` allows for streaming logs from within the enclave. This is intended for debugging issues related to attestations and communicating with services outside the enclave, and not for general debugging. For debugging during development, it is more useful to run your container directly outside of an enclave.

Refer to the [full list of commands][cmd-run] to learn about all of the features.

## Enclaver Image Format

The Enclaver image format is a regular OCI container image with a specific layout, roughly split into the inside and outside components:

```sh
TODO: add image structure in style of `ls -la`
/enclave/application.eif
/enclave/policy.yaml
```

The inner components inside `/enclave` are placed inside of another format, the Nitro-compatible Enclave Image Format (EIF) file. The EIF is an Amazon specification, and looks similar to an AMI, since it contains a kernel and Linux userland. Enclaver vendors Amazon's `nitro-cli` code to build the EIF. Enclaver will override the `ENTRYPOINT` of your source container with it's own PID1 and then trigger your original entrypoint. Do not plan to pass runtime configuration into the enclave.

The network policy is duplicated in two places: inside of the EIF so it's part of the [cryptographic attestation][attestation] and outside so it can be read by other components outside of the enclave.

### Calculating Cryptographic Attestations

`enclaver trust` outputs the cryptographic attestation of an image. An attestation is a reproducable "measurement" of a piece of code that can be used to give the code a unique identity. The word "measurement" is used because, just like a ruler, Enclaver records the content of various parts of the code that make up the enclave image. The hash of this measurement is recorded into Platform Configuration Registers (PCRs). A collection of certain PCRs (eg. PCR0-4 + PCR8) is the unique attestation of that particular piece of code.

```sh
$ enclaver trust registry.example.com/my-app:v1.0
{
  "Measurements": {
    "HashAlgorithm": "Sha384 { ... }",
    "PCR0": "7fb5c55bc2ecbb68ed99a13d7122abfc0666b926a79d5379bc58b9445c84217f59cfdd36c08b2c79552928702efe23e4",
    "PCR1": "235c9e6050abf6b993c915505f3220e2d82b51aff830ad14cbecc2eec1bf0b4ae749d311c663f464cde9f718acca5286",
    "PCR2": "0f0ac32c300289e872e6ac4d19b0b5ac4a9b020c98295643ff3978610750ce6a86f7edff24e3c0a4a445f2ff8a9ea79d",
    "PCR8": "70da58334a884328944cd806127c7784677ab60a154249fd21546a217299ccfa1ebfe4fa96a163bf41d3bcfaebe68f6f"
  }
}
```

An attestion must be reproduceable in order to ensure that each time the enclave is started, and nothing about it has been modified, the attestation remains constant. Other systems will rely on this reproduceability to build trust with the software. 

If an attestation matches, engineers can guarantee that its configuration is _exactly_ what was tested/verified/trusted. If you're remotely communicating with an enclave, the attestation can remotely prove it's configuration. An attestation serves as a piece of identity that can't be hijacked because it's crypographically tied to the code itself.

PCR0 measures the length of our EIF file. Since it's critical that our enclave code is not modified, this is important to check.
PCR1 measures our enclave's kernel and boot RamFS data. Again, we don't want our kernel image modified or the kernel parameters changed.
PCR3 measures the IAM instance role assigned to your EC2 machine.
PCR4 measures the instance ID of a specific EC2 machine.
PCR8 measures the enclave image file signing certificate.

When using attestations, you must decide which "measurements", the PCR values, are useful to you. In almost all cases you should care about the image and kernel parameters, but if you're running multiple enclave instances with the same code, PCR4 will not be useful because it will be unique to each EC2 machine running your enclaves.

Enclaver's `trust` command uses PCR0, PCR1, PCR2, PCR8 to calculate its attestation before execution. Because it's not possible to know PCR3 until the machine is running, it is most useful when configuring an AWS Key Management Service (KMS) policy.

From within the enclave, the `get-attestation-document` API also provides the ability for your code to query its own attestation document.

## Components Outside the Enclave

The goal of components outside of the enclave are to monitor the health of the enclave and to route allowed traffic into the enclave. Since isolation is a critical component to enclave security, Enclaver has proxies sitting on both sides of the virtual socket (vsock) that connects the inside and outside.

These components have minimal overhead, in the order of xx MB of RAM and xx millicores of CPU (TODO: insert), in addition to the CPU and RAM carved out for the enclave itself.

### Enclave Supervisor

`enclaver run` is the enclave supervisor. It runs as a systemd unit and exits if the enclave dies. By design, there is very little visibility into the enclave, so the command watches the context ID (CID) for information provided directly from the Nitro hypervisor.

```yaml
[Unit]
Description=Enclaver
Documentation=https://edgebit.io/enclaver/docs/
After=docker.service
Requires=docker.service

[Service]
TimeoutStartSec=0
Restart=always
ExecStartPre=-/usr/bin/docker exec %n stop
ExecStartPre=-/usr/bin/docker rm %n
ExecStartPre=/usr/bin/docker pull us-docker.pkg.dev/edgebit-containers/containers/enclaver:v0.1.0
ExecStart=/usr/bin/docker run \
    --rm \
    --name %n \
    --privileged \
    -p 443:443 \
    us-docker.pkg.dev/edgebit-containers/containers/enclaver:v0.1.0 run \
    registry.example.com/app-enclave:v1

[Install]
WantedBy=multi-user.target
```

### Outer Proxy

The outer proxy sets up routing from the rest of your AWS infrastructure into the enclave. The other end of the virtual socket is running within the trusted environment, which protects against a malicious outer proxy and enforces the enclave's network policy.

The outer proxy only forwards HTTP and TCP traffic into the enclave.

If the enclave is running in debug mode, the outside proxy allows for streaming logs through the virtual socket for debugging.

## Components Inside the Enclave

The goal inside of the enclave is to protect your workload from the outside world. A single component, named `odyn`, provides all of the inner functionality.

### Process Supervisor

Enclaver runs the supervisor as PID2 (soon to be PID1) inside the enclave to accomplish:

1. Enclave bootstrap - bring up loopback and seed entropy
1. Execute the original `ENTRYPOINT` from your container
1. Provides the entrypoint status to the outside
1. Forwards the logs to the outside
1. Reaps zombies (disabled until running as PID1)

### Inner Proxy

The inner proxy provides routing to the outside world and does network filtering based on the policy baked into the enclave image. This protects your code from outside network based attacks and is a layer of defense against exfiltration of data caused by a vulnerability in a library inside the enclave.

For ingress, TLS is terminated and ingress policy is enforced. The private keys used for TLS termination are fetched from KMS.

For egress, policy is enforced before traffic leaves the enclave.

The inner proxy also appends the attestation of the enclave to `Decrypt`, `GenerateDataKey`, and `GenerateRandom` calls to AWS KMS, which allows for super easy integration for your code to use your KMS keys to decrypt data within the enclave. This is when you see the power of using the output from `enclaver trust --kms` as part of a KMS key policy.

TODO: upadte with final enclaver trust command

### Verifying Cryptographic Attestations

TODO: does this need to be captured as an issue?

`enclaver run --verify-before-run attestation.json` will verify an attestation of an image after fetching it, but before executing it. If the comparison fails, the violating PCRs will be logged and the command will fail with an exit code. Since our threat model can consider the host hostile, this is more of a corruption check.

Inside of the enclave, the KMS proxy will also fetch the attestation, but it will come directly from the hypervisor, so it can be fully trusted. The `get-attestation-document` API  is only available inside of the enclave.

TODO: expand general usage with other non-KMS systems


TODO: update header links

[cli]: #enclaver-cli
[format]: #enclaver-image-format
[outside]: #components-outside-the-enclave
[inside]: #components-inside-the-enclave
[attestation]: #calculating-cryptographic-attestations
[cmd]: commands.md
[cmd-run]: commands.md#run
[cmd-build]: commands.md#build