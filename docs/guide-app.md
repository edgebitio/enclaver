---
title: "Running a Python App"
layout: docs-enclaver-single
category: guides
---

## Running a Python app inside an Enclave

TODO: add set up for the app

For this example you’ll need an EC2 instance with support for Nitro Enclaves enabled (c5.xlarge is the cheapest qualifying instance type as of this writing) and Docker installed.  See [the Deploying on AWS](deploy-aws.md) for more details.

## Create the Policy

TODO: add policy

## Build the Enclave Image

TODO: add build command

## Run the Enclave

TODO: add run command

## Next Steps

This example walked through running a simple Python app that represented running a specific microservice or a security-centric function within an enclave. It's also possible to run an entire application in an enclave to wrap it in a higher level of security.

Check out [running Hashicorp Vault](guide-vault.md) for a walkthrough.