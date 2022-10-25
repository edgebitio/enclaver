---
title: "Running Hashicorp Vault in an Enclave"
layout: docs-enclaver-single
category: guides
---

# Running Hashicorp Vault in an Enclave

Enclaver works by transforming a containerized application into a new container image which runs the underlying application in an enclave.

To demonstrate how this works, let’s run HashiCorp Vault in an enclave. Vault has good security out of the box, but if we trust our attestation, we can cryptographically guarantee that our Vault will only be unsealed in a known trusted state and all material can’t be leaked outside the enclave.

For this example you’ll need an EC2 instance with support for Nitro Enclaves enabled (`c6a.xlarge` is the cheapest qualifying instance type as of this writing) and Docker installed.  See [the Deploying on AWS](deploy-aws.md) for more details.

[![CloudFormation](img/launch-stack.svg)][cloudformation]

[cloudformation]: https://us-east-1.console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/create/review?templateURL=https://enclaver-cloudformation.s3.amazonaws.com/enclaver.cloudformation.yaml&stackName=Enclaver-Demo

## Create the Manifest

Enclaver uses a [manifest file][manifest] to define the contents of an enclave. Let’s start by creating a file called `enclaver.yaml`.

```yaml
TODO: add me
```

While we aren't modifying Vault at all, our source container is a standard Vault container with some TLS configuration from AWS KMS and embedded unseal configuration ([view Dockerfile][dockerfile]).

TODO: add dockerfile link

For storage, we will run Consul outside of the enclave.

## Build the Enclave Image

Now, we’ll ask Enclaver to build a new container image using this policy:

TODO: add real container and PCRs

```sh
$ enclaver build -f enclaver.yaml
 INFO  enclaver::images > latest: Pulling from edgebit-containers/containers/vault-enclave
 INFO  enclaver::images > latest: Pulling from edgebit-containers/containers/odyn
 INFO  enclaver::images > latest: Pulling from edgebit-containers/containers/nitro-cli
 INFO  enclaver::build  > starting nitro-cli build-eif in container: 40bcc4af5c0581c5fb6fc04e2aef4458b326738c7938e08df19244ec3c847972
 INFO  nitro-cli::build-eif > Start building the Enclave Image...
 INFO  nitro-cli::build-eif > Using the locally available Docker image...
 INFO  nitro-cli::build-eif > Enclave Image successfully created.
 INFO  enclaver::build      > packaging EIF into release image
Built Release Image: sha256:da0dea2c7024ba6f8f2cb993981b3c4456ab8b2d397de483d8df1b300aba7b55 (vault-enclave:enclave-latest)
EIF Info: EIFInfo {
    measurements: EIFMeasurements {
        pcr0: "b3c972c441189bd081765cb044dfcf69da0f57050474fb29e8f4f3d4b497cd66567f3f39935dee75d83ea0c9e9483d5a",
        pcr1: "bcdf05fefccaa8e55bf2c8d6dee9e79bbff31e34bf28a99aa19e6b29c37ee80b214a414b7607236edf26fcb78654e63f",
        pcr2: "40bf9153c43454574fa8ff2d65407b43b26995112db4e1457ba7f152b3620d2a947b0e595d513cb07f965b38bf33e5df",
    },
}
```

If you want to avoid building your own image, you can use `us-docker.pkg.dev/edgebit-containers/containers/vault-enclave:enclave-latest`.

In this step, the Enclaver build process:

1. Built a new Docker image, based on the source image and injecting several Enclaver-specific components:
  - the Enclaver internal supervisor + proxy binary
  - the manifest file we provided, including network policy
2. Converted this Docker image into a Nitro Enclaves compatible EIF file
3. Built a new container image which bundles the Enclaver “external proxy” and Nitro Enclave launcher.

If you are curious about these components, read about the [architecture](architecture.md).

## Configure Auto-Unsealing

Vault supports unsealing from a KMS key. We are going to increase security of this feature by protecting that KMS key with an access policy that only allows unsealing from inside of an enclave. This prevents anyone, whether insider or attacker, from accessing our instance of Vault and removes any possibility of viewing the master key in plaintext.

In our build step, you will see that some "measurements" are displayed. These are a [cryptographic attestation][attestation], an identity for the code, that can't be spoofed or stolen by an attacker. Our key policy will contain some of these values, which prove that only _this specific code_ can access the unseal key.

Here's an example key policy that should be attached to your instance role as the `Principal`. Modify the PCR2 value if you didn't use the prebuilt image.

```
{
    "Version": "2012-10-17",
    "Id": "key-vault",
    "Statement": [
        {
            "Sid": "Allow unsealing of Vault from enclave",
            "Effect": "Allow",
            "Principal": {
                "AWS": "arn:aws:iam::<your ARN>"
            },
            "Action": [
                "kms:Decrypt",
                "kms:DescribeKey"
            ],
            "Resource": "*",
            "Condition": {
                "StringEqualsIgnoreCase": {
                    "kms:RecipientAttestation:PCR2": "40bf9153c43454574fa8ff2d65407b43b26995112db4e1457ba7f152b3620d2a947b0e595d513cb07f965b38bf33e5df"
                }
            }
        }
    ]
}
```

## Run the Enclave

The result of the build step is a new Docker image named `vault-enclave`. Running this image starts up Vault in a Nitro Enclave on your AWS machine!

First, SSH to your EC2 machine:

```sh
$ ssh ec2-user@<ip address>
```

### Start Consul

Next, start Consul inside of a container:

```sh
$ docker run \
  --name consul
  --detached \
  --rm \
  -p 8500:8500 \
  -p 8600:8600/udp \
  consul
```

Make sure it's happy:

```
$ docker logs consul
```

### Start the Enclave

Start the enclave. We will manually use Docker, but you can also set up a [systemd unit][unit].

```sh
$ docker run vault-enclave
TODO: add output
```

You should see output confirming that the enclave started, fetched the unseal key, and used it to unseal Vault:

```
$ docker logs enclave
TODO: add output
```

## Using Vault

TODO: add short command to show vault working normally

## Next Steps

This example walked through running an entire application in an enclave. Next, experiment with running a specific microservice or a security-centric function within an enclave.

Check out the [example Python app][guide-app] for a walkthrough.

[manifest]: manifest.md
[dockerfile]: TODO-add-me
[attestation]: architecture.md#calculating-cryptographic-attestations
[unit]: deploy-aws.md#run-via-systemd-unit
[guide-app]: guide-app.md