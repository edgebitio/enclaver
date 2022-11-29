---
title: "Run Your First Enclave"
layout: docs-enclaver-single
category: guides
weight: 1
---

# Build and Run Your First Enclave

This guide will walk you through booting an EC2 machine, configuring it, then building and running your first enclave with Enclaver.

## Boot the Machine

Only certain [EC2 instance types][instance-req] can run Nitro Enclaves. Since dedicated CPUs are carved off for the enclave, the larger x86 types with 4+ vCPUs are ok, with `c6a.xlarge` being the cheapest. Your machines must be booted with the Nitro Enclave option enabled, which is found under the "Advanced details" > "Nitro Enclave". 

See [the Deploying on AWS](deploy-aws.md) for more details and an example CloudFormation template.

## Configure Instance

First, SSH to your EC2 machine:

```sh
$ ssh ec2-user@<ip address>
```

Install the Nitro Enclave packages:

```
$ sudo amazon-linux-extras install aws-nitro-enclaves-cli -y
$ sudo yum install aws-nitro-enclaves-cli-devel git -y
```

Configure the resources to dedicate to your enclaves:

```
$ sudo sed -i 's/memory_mib: 512/memory_mib: 3072/g' /etc/nitro_enclaves/allocator.yaml
$ sudo systemctl start nitro-enclaves-allocator.service && sudo systemctl enable nitro-enclaves-allocator.service
$ sudo systemctl start docker && sudo systemctl enable docker
```

Clone Enclaver, which contains the example app code:

```sh
$ git clone https://github.com/edgebitio/enclaver
```

Download the latest Enclaver binary:

```sh
$ curl -sL $(curl -s https://api.github.com/repos/edgebitio/enclaver/releases/latest | jq -r '.assets[] | select(.name|match("^enclaver-linux-x86_64(.*)tar.gz$")) | .browser_download_url') --output enclaver.tar.gz
$ tar -xvf enclaver.tar.gz
$ chmod +x enclaver-linux-x86_64-v0.2.0/enclaver
$ sudo cp enclaver-linux-x86_64-v0.2.0/enclaver /usr/bin/enclaver
```

## Build the App

Enclaver uses a source "app" container image and transforms that image into an enclave image. Build the source app:

```sh
$ cd enclaver/example
$ sudo docker build -t app .
```

This app echos a string back to you with each HTTP request:

```sh
$ sudo docker run --rm -d --name app -p 8000:8000 app
$ curl localhost:8000
Hello World!
$ sudo docker stop app
```

## Build the Enclave Image

Enclaver builds enclave images based on a configuration file, which specifies the container that holds the app code, the network policy details.

Here's the `enclaver.yaml` for the example app:

```yaml
version: v1
name: "example"
target: "enclave:latest"
sources:
  app: "app:latest"
defaults:
  memory_mb: 1000
egress:
  - host
ingress:
  - listen_port: 8000
```

Build our enclave image:

```sh
$ sudo enclaver build --file enclaver.yaml
```

## Run the Enclave

Run your enclave image by referencing it by it's given `target` name:

```sh
$ sudo enclaver run -p 8000:8000 enclave:latest
```

Open a new shell and send a request to the service:

```sh
$ curl localhost:8000
Hello World!
```

In your first shell you should see that the enclave received the request:

```
 INFO  enclave         > Example app listening on port 8000
 INFO  enclave         > Request received!
```

Boom! You've started your first secure enclave!

To clean up, you can `^C` the `enclaver run` and it will shut down.

## Next Steps

This example walked through running an simple application in an enclave. Next, experiement with running a specific microservice or a security-centric function within an enclave.

Check out the [example Python app that talks to KMS](guide-app.md).
