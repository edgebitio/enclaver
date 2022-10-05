---
title: "Running Hashicorp Vault in an Enclave"
layout: docs-enclaver-single
category: guides
---

# Running Hashicorp Vault in an Enclave

Enclaver works by transforming a containerized application into a new container image which runs the underlying application in an enclave.

To demonstrate how this works, let’s run HashiCorp Vault in an enclave. Vault has good security out of the box, but if we trust our attestation, we can cryptographically guarantee that our Vault will only be unsealed in a known trusted state and all material can’t be leaked outside the enclave.

For this example you’ll need an EC2 instance with support for Nitro Enclaves enabled (c5.xlarge is the cheapest qualifying instance type as of this writing) and Docker installed.  See [the Deploying on AWS](deploy-aws.md) for more details.

## Create the Policy

Enclaver uses a declarative policy file to define the contents of an enclave. Let’s start by creating a policy file called `policy.yaml`.

```yaml
version: v1
image: "vault:1.11.2"
name: "enclaver-vault"
resources:
  cpus: 2
  memory: 1024
network:
  listen_ports:
    - 8080
  egress:
    - host: 169.254.169.254
      port: 80
    - host: kms.us-west-2.amazonaws.com
      port: 443
    - host: google.com
      port: 443
    - host: www.google.com
      port: 443
```

TODO: add support to Enclaver, and update the above policy, to support:
TODO:   Injecting a config file
TODO:   Specifying additional runtime flags to Vault
TODO:   Configuring in-enclave KMS proxy

## Build the Enclave Image

Now, we’ll ask Enclaver to build a new container image using this policy:

```sh
$ enclaver build -f policy.yaml
building overlay layer for source image
overlay completed, saving overlaid image
overlaid image saved as bfbd5765-a430-4047-b0fb-dd0cc7c9ff86
overlaying EIF and policy file onto base wrapper image
saving completed wrapper image to local docker daemon
successfully built image: example-enclave
EIF Image Sha384: 4d4e7d5d6e00fbd2d8ae9ebbfeb067a5ebccb31a049e133eb183938c1cdbc2ef8708151c9d6292f4a2c27c8dc4cef014
```

In this step, the Enclaver build process:

1. Built a new Docker image, based on the source `vault` image and injecting several Enclaver-specific components:
 - the Enclaver internal supervisor + proxy binary
 - the policy file we provided
2. converted this Docker image into a Nitro Enclaves compatible EIF file
3. built a new container image which bundles the Enclaver “external proxy” and Nitro Enclave launcher.

If you are curious about these components, read about the [architecture](architecture.md).

## Run the Enclave

The result of the build step is a new Docker image named `enclaver-vault`. Running this image starts up Vault in a Nitro Enclave on your AWS machine!

On your EC2 machine, start the enclave with Enclaver:

```sh
$ enclaver run enclaver-vault
TODO: add output
```

TODO: add details about introspection of the running enclave
TODO:   state
TODO:   log streaming

## Using Vault

TODO: add short command to show vault working normally

## Next Steps

This example walked through running an entire application in an enclave. Next, experiement with running a specific microservice or a security-centric function within an enclave.

Check out the [example Python app](guide-app.md) for a walkthrough.