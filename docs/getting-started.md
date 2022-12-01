---
title: "Getting Started with Enclaver"
layout: docs-enclaver-single
aliases:
    - /enclaver/docs/
    - /enclaver/docs/0.x/
---

# Getting Started with Enclaver

Enclaver is an open source toolkit created to enable easy adoption of software enclaves, for new and existing backend software.

Enclaves provide several critical features for operating software which processes sensitive data, including isolation, attestation and network restrictions.

![Enclaver Architecture Diagram](img/diagram-enclaver-components.svg)

Refer to [the architecture](architecture.md) for a complete understanding of Enclaver components.

[![Enclaver demo on YouTube](img/thumb-run.png)](https://www.youtube.com/watch?v=nxSgRYten1o)

## Tutorials

### [Run Your First Enclave][first]

Build and run your first enclave. All you need is a new EC2 machine and we'll walk through everything else.

### [No-Fly-List Python + KMS app][no-fly-app]

Deploy the No-Fly-List app, which checks passengers attempting to fly on an airline against a no-fly list. It’s a fairly simple Python application that requires protection “in-use” for its data, because we don’t want anyone to be able to see the full no-fly list.

This guide is applicable to any microservice or security-centric function at your organization.

### [Hashicorp Vault][vault]

Run Hashicorp Vault within an enclave to fully isolate it after it's unsealed.

This guide is model for running off-the-shelf or commercial software in an enclave.

### [Deploy on AWS][aws]

Straightforward guide to getting started with Enclaver on AWS with EC2 machines that are enabled to run Nitro Enclaves.

[first]: guide-first.md
[no-fly-app]: guide-app.md
[vault]: guide-vault.md
[aws]: deploy-aws.md