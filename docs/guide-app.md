---
title: "Running a Python App"
layout: docs-enclaver-single
category: guides
---

# Running the No-Fly-List app inside an Enclave

This guide will walk through running the No-Fly-List app, which checks passengers attempting to fly on an airline against a no-fly list. It's a fairly simple Python application that requires protection "in-use" for it's data, because we don't want anyone to be able to see the full no-fly list. The app uses a secure enclave and Amazon KMS to achieve that.

<details>
  <summary>How does the enclave's isolation protect the no-fly list?</summary>

The enclave is extremely isolated by design. It guarantees that no one can inspect it's memory, interactively log in to it, or read data inside of it. This makes it a safe place to decrypt our data and run the passenger matching logic against it. When auditing the code, we can see that it returns our allowed or denied message and nothing more. There are no avenues where the no-fly list can escape.
</details>

<details>
  <summary>How does the enclave's code identity protect the no-fly list?</summary>

The enclave has a code identity, called an attestation, that unqiuely identifies it. This is generated through cryptography, so it's impossible to fake. Since it's code, it's not possible to steal, unlike a human's identity.

`enclave trust` will show you the cryptographic attestation of a specific enclave image. Our specific code, via the attestation, is granted access to read our encryption key needed to decrypt the no-fly list. With a locked-down access policy, it's _impossible_ for anything other than _this specific code_ to read the key.

Here's an example of an attestation:

TODO: Implement trust command. See [issue #38](https://github.com/edgebitio/enclaver/issues/38).

```sh
$ enclaver trust us-docker.pkg.dev/edgebit-containers/containers/no-fly-list:enclave-latest
TODO: add real attestation
```
</details>

In the recent past, there was an incident  – this is a rumor – that caused the entire cast of Sesame Street to be added to the no-fly list. We can find out if that's true :)

For this example you’ll need an EC2 instance with support for Nitro Enclaves enabled (`c6a.xlarge` is the cheapest x86 instance) and Docker installed.  See [the Deploying on AWS](deploy-aws.md) for more details.

[![CloudFormation](img/launch-stack-x86.svg)][cloudformation-x86]
[![CloudFormation](img/launch-stack-arm.svg)][cloudformation-arm]

[cloudformation-x86]: https://us-east-1.console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/create/review?templateURL=https://enclaver-cloudformation.s3.amazonaws.com/enclaver.cloudformation-x86.yaml&stackName=Enclaver-Demo
[cloudformation-arm]: https://us-east-1.console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/create/review?templateURL=https://enclaver-cloudformation.s3.amazonaws.com/enclaver.cloudformation-arm.yaml&stackName=Enclaver-Demo

## The No-Fly-List App

On startup, the app fetches an encrypted blob from S3. This blob is encrypted with a scheme called "envelope encryption". Let's take a quick detour to understand it, because it's both interesting and really useful.

### Envelope Encryption

Envelope Encryption involves encrypting our data twice, once at the app-level with key A, and then encrypting key A with key B. In our app, we have the data that was encrypted with key A (the data key), and an encrypted version of key A. This is our "envelope", because we've wrapped our data with the second level of encryption using key B (master key).

```json
{
  "key-A-ciphertext": "UhOUXl...besRT=",
  "data-ciphertext":  "QZQE2J...uypwE="
}
```

<details>
  <summary>Why do we use Envelope Encryption?</summary>

If we throw away the plaintext of key A, the envelope only contains encrypted data, so it can be stored in a database or sent through microservices safely. If a consumer down the line needs to decrypt it, they can decrypt key A if they have access to key B. The process unlocks numerous benefits:

1. Data keys (key A) can be used to encrypt multiple files or pieces of data received at the same time. It's common to have a database row that has multiple fields encrypted with a single key ("field-level encryption"):
    | ID | Encrypted Data Key | First Name | Last Name | Address |
    |----|--------------------|------------|-----------|---------|
    | 1  |UhOUXlT2an029Xqva...|kDAgEQgDu...|vQFPsDGU...|QE2J4n...|
1. Encrypted data keys are stored with the data, so there isn't additional key management needed to store and track them.
1. Envelope encryption normalizes using unique keys for your data instead of one single key that can be compromised.
1. Different encryption schemes can be used to maximize performance. Data keys typically need to encrypt large objects, so they need to be performant and use symmetric key algorithms. Using public key algorithms are slower but carry the convinience of the public/private key separation. Envelope encryption allows using both schemes where each are well-suited for the task.  
1. Possible to encrypt the same data key under multiple master keys without re-encrypting the raw data.
</details>

Functionally, this means we'll have two layers of encryption, the inner layer is normal AES symmetric encryption around our sensitive data and the outer layer of public/private key encryption with the private key stored in AWS KMS.

Here's what the actual envelope used by the app looks like:

```json
{
  "data-key-ciphertext-base64": "AQIDAHgFXi2TEB5uhOUXl62UNxtALVzp0EqotGT2an02XqvQvQFPsDGUMVCbesRTBymyEYYBAAAAfjB8BgkqhkiG9w0BBwagbzBtAgEAMGgGCSqGSIb3DQEHATAeBglghkgBZQMEAS4wEQQMn/xg+FHWxztbsikDAgEQgDulQ5ROICb+58HcwXTls2bUohxdxN4FFZnp4QFbAweKGJwEEmhkNp7HnrQU+wPUXvQVc7m+bPVeoXksSQ==",
  "aes-ciphertext-base64": "U2FsdGVkX18a+Ji6uIdSAc9GQF3BV1EqZQE2J42nOJUxiyDJr12mSXI2qm5Z5no1KZGM4dKeuBSDwQuyOCJrpwE0g6+XERruQLdazh02Vq3VLx5MwaM7pVBwJLXlt6Wnl8HWtXNjNCySQKrMJmUIH+arCxthxUho4ABiNZ+nJEW3+GEYsmD92KcK/CzytFJVH6X8QajJn4kq5dbMa6rDxw=="
}
```

### Decrypting the No-Fly List

Inside of the enclave, we have a policy that allows us to decrypt the data key. Since it's recorded in the file, it's easy to know which one to request from KMS. When we get that, we can use it to decrypt the main part of our file.

While we don't explore it here, the powerful part is that we could locally decrypt large or numerous files without having to transmit the ciphertext through KMS, because we have the plaintext data key already. The isolation guarantees of the enclave make it safe to cache the plaintext data key, unlike if you were keeping it in RAM or on disk in a regular VM. 

The concept of multiple data keys is called "field level encryption" and passing around these envelopes around is "app-level encryption".

Hope you learned something! Let's jump into deploying our app.

## Create the Enclave Configuration

Enclaver builds enclave images based on a configuration file, which specifies the container that holds the app code, the network policy for egress, and a few other details.
This policy is packaged into the image because it is distributed with the image and included in its [cryptographic attestation][attestation].

Here's the configuration for the No-Fly list app:

```yaml
version: v1
name: "test"
target: "no-fly-list:enclave-latest"
sources:
  app: "us-docker.pkg.dev/edgebit-containers/containers/no-fly-list:latest"
defaults:
  memory_mb: 4096
kms_proxy:
  listen_port: 9999
egress:
  allow:
    - kms.*.amazonaws.com
    - s3.amazonaws.com
    - 169.254.169.254
ingress:
  - listen_port: 8001
```

It's pretty straightforward. The `sources.app` parameter specifies the source container for our code. Since we're using AWS KMS for crypotgraphy and S3 for fetching our encrypted no-fly list, those addresses are allowed. The IP address is the AWS istance metadata service, where we get a dynamic set of credentials to use for the KMS and S3 requests.

[attestation]: architecture.md#calculating-cryptographic-attestations

## Build the Enclave Image

`enclaver build` takes an existing container image of your application code and builds it into a new container image with enclave-specific components added in. This is what we'll run on our EC2 machine. This image can be pushed to a registry like any other container.

We're passing in our manifest file from above to the build:

```
$ enclaver build --file enclaver.yaml
 INFO  enclaver::images > latest: Pulling from edgebit-containers/containers/no-fly-list
 INFO  enclaver::images > latest: Pulling from edgebit-containers/containers/odyn
 INFO  enclaver::images > latest: Pulling from edgebit-containers/containers/nitro-cli
 INFO  enclaver::build  > starting nitro-cli build-eif in container: 40bcc4af5c0581c5fb6fc04e2aef4458b326738c7938e08df19244ec3c847972
 INFO  nitro-cli::build-eif > Start building the Enclave Image...
 INFO  nitro-cli::build-eif > Using the locally available Docker image...
 INFO  nitro-cli::build-eif > Enclave Image successfully created.
 INFO  enclaver::build      > packaging EIF into release image
Built Release Image: sha256:da0dea2c7024ba6f8f2cb993981b3c4456ab8b2d397de483d8df1b300aba7b55 (no-fly-list:enclave-latest)
EIF Info: EIFInfo {
    measurements: EIFMeasurements {
        pcr0: "b3c972c441189bd081765cb044dfcf69da0f57050474fb29e8f4f3d4b497cd66567f3f39935dee75d83ea0c9e9483d5a",
        pcr1: "bcdf05fefccaa8e55bf2c8d6dee9e79bbff31e34bf28a99aa19e6b29c37ee80b214a414b7607236edf26fcb78654e63f",
        pcr2: "40bf9153c43454574fa8ff2d65407b43b26995112db4e1457ba7f152b3620d2a947b0e595d513cb07f965b38bf33e5df",
    },
}
```

## Run the Enclave

`enclaver run` executes on an EC2 machine to fetch, unpack and run your enclave image. First, SSH to your EC2 machine:

```sh
$ ssh ec2-user@<ip address>
```

After the image is fetched, it is broken apart into [the outside][outside] and [inside components][inside]. The outer components are started first, then the enclave, with the inner components inside, is started.

We will start it manually using Docker, but you can also set up a [systemd unit][unit].

```sh
$ docker run \
    --rm \
    --detach \
    --name enclave \
    --device=/dev/nitro_enclaves:/dev/nitro_enclaves:rw \
    -p 8001:8001 \
    us-docker.pkg.dev/edgebit-containers/containers/no-fly-list:enclave-latest
```

Check to see that the enclave was run successfully:

```sh
$ docker logs enclave
 INFO  enclaver::run   > starting egress proxy on vsock port 17002
 INFO  enclaver::vsock > Listening on vsock port 17002
 INFO  enclaver::run   > starting enclave
 INFO  enclaver::run   > started enclave i-00e43bfc030dd8469-enc1840fa584262e1a
 INFO  enclaver::run   > waiting for enclave to boot
 INFO  enclaver::run   > connected to enclave, starting log stream
 INFO  enclave         >  INFO  enclaver::vsock > Listening on vsock port 17001
 INFO  enclave         >  INFO  enclaver::vsock > Listening on vsock port 17000
 INFO  enclave         >  INFO  odyn::enclave   > Bringing up loopback interface
 INFO  enclave         >  INFO  odyn::enclave   > Seeding /dev/random with entropy from nsm device
 INFO  enclave         >  INFO  odyn            > Enclave initialized
 INFO  enclave         >  INFO  odyn            > Startng egress
 INFO  enclave         >  INFO  odyn            > Startng ingress
 INFO  enclave         >  INFO  enclaver::vsock > Listening on vsock port 8001
 INFO  enclave         >  INFO  odyn            > Starting KMS proxy
 INFO  enclave         >  INFO  odyn::kms_proxy > Generating public/private keypair
 INFO  enclave         >  INFO  enclaver::vsock > Connection accepted
 INFO  enclave         >  INFO  enclaver::vsock > Connection accepted
 INFO  enclave         >  INFO  odyn::kms_proxy > Fetching credentials from IMDSv2
 INFO  enclave         >  INFO  odyn::kms_proxy > Credentials fetched
 INFO  enclave         >  INFO  odyn            > Starting ["python", "-m", "flask", "run", "--host=0.0.0.0", "--port=8001"]
 INFO  enclave         >  * Serving Flask app "/opt/app/server.py"
 ...app logs...
```

[unit]: deploy-aws.md#run-via-systemd-unit
[outside]: architecture.md#components-outside-the-enclave
[inside]: architecture.md#components-inside-the-enclave

## Submit Passenger Names

Now the fun part. Let's see who can fly and who can't. Remember, a key part of this scenario is that no one should be able to see the complete no-fly list, but we should get back an answer for each person when they buy a ticket.

We know that members of Sesame Street might not be allowed to fly. Test it out for yourself from the EC2 machine:

```sh
$ curl localhost:8001/enclave/passenger?name=foo
foo is cleared to fly. Enjoy your flight!
```

See how many names you can discover that won't be flying today.

### Dedicated CPUs

If you booted a `c6a.xlarge`, the full machine has 4 vCPUs. By default, Enclaver dedicates 2 of those to the Nitro Enclave. Dedicated CPUs are part of the isolation and protection of your workloads in the enclave.

You can test this out by running `top` and then hitting `1` to show a breakdown of CPUs. Notice that CPUs `Cpu1` and `Cpu3` are missing here:

```
top - 14:48:27 up 6 min,  1 user,  load average: 0.01, 0.06, 0.03
Tasks: 111 total,   1 running,  52 sleeping,   0 stopped,   0 zombie
%Cpu0  :  0.3 us,  0.0 sy,  0.0 ni, 99.7 id,  0.0 wa,  0.0 hi,  0.0 si,  0.0 st
%Cpu2  :  0.3 us,  0.0 sy,  0.0 ni, 99.7 id,  0.0 wa,  0.0 hi,  0.0 si,  0.0 st
KiB Mem :  7949808 total,  6388256 free,   669472 used,   892080 buff/cache
KiB Swap:        0 total,        0 free,        0 used.  7046532 avail Mem
```

## Check out the Code

This application is on GitHub: https://github.com/edgebitio/no-fly-list/blob/main/server.py

Once you factor out the S3 fetching and the boilerplate KMS handling, our actual logic is just a [handfull of lines][code] that is easily audited. This is the ideal type of enclave app. It's focused, simple and acts like a secure sidecar to the rest of our app.

[code]: https://github.com/edgebitio/no-fly-list/blob/main/server.py#L56-L65

## Next Steps

This example walked through running a simple Python app that represented running a specific microservice or a security-centric function within an enclave. It's also possible to run an entire application in an enclave to wrap it in a higher level of security.

Check out [running Hashicorp Vault](guide-vault.md) for a walkthrough.