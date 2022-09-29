---
title: "FAQ"
layout: docs-enclaver-single
category: getting_started
weight: 10
---

## Enclaver FAQ

### What threats does Enclaver protect against?

Enclaver (and all secure enclaves) guarantees that any sensitive data inserted, processed or decrypted by the enclave can never be read by an attacker nor can it leave the enclave unless explictly allowed. This is the basis for the [“magic formula”](https://edgebit.io/blog/introducing-edgebit/#the-magic-formula) – trusted runtime, network policy, app code and identity.

In short, if you handled 100% of your plaintext or sensitive data within an enclave, you should be protected from an attacker that has exploited the infrastructure hosting the enclave. This is just like an attacker stealing your iPhone – they can't obtain your fingerprint or FaceID.

It is your responsibility to ensure that only encrypted/hashed/tokenized data or summaries and non-sensitive result sets leave the enclave. Enumeration attacks are possible, based on what your code returns to requesters. Overall, the risk to your crown jewels, like encryption keys, is dramatically lower than using a regular virtual machine workflow.

Specifically Enclaver is focused on these threat reductions:

 - Insider Threats
   - Prevent an engineer debugging production from encountering sensitive data in memory or on disk
   - Prevent stolen production credentials from reading sensitive data in memory or on disk ([Twilio/Signal attack][twilio], [Uber attack][uber])
 - Application or infrastructure compromise
   - Prevent an attacker who is able to [dump application memory][heartbleed] from accessing sensitive data
   - Prevent modification of the enclave code from its trusted attestation after boot
 - Unwanted Data Ingress
   - Operate as a sidecar over vsock (basically localhost) or connect to a specific network interface
 - Unwanted Data Egress
   - Allow list of hostnames that the enclave can communicate with
   - Prevent data egress due to coding or logic bugs
   - Prevent data egress due to exploit (Log4j)
 - Supply Chain Attacks
   - Prevent modification of enclave code from its trusted attestation prior to boot
   - Prevent enclave from reading cryptographic material from that doesn't match the trusted attestation (KMS policy)
   - Prevent data egress due to coding or logic bugs present in software dependencies out of your control
 - Reduced scope and footprint
   - Prevent chained attacks that rely on large amounts of software dependencies

Take care in returning data from the enclave to external parties and ensure your attestations are verfied, audited and specific.
TODO: make this closing more specific

[twilio]: https://edgebit.io/blog/threatvector-twilio-signal/
[uber]: https://edgebit.io/blog/threatvector-uber/
[heartbleed]: https://heartbleed.com/

### How is Enclaver different than using Amazon's Nitro Enclave tools?

Existing tools for working with Nitro Enclaves provide awesome building blocks, but actually using those building blocks to run code in an enclave is challenging.

Enclaver makes it simple to put applications into Nitro Enclaves by automatically doing most of the heavy lifting, while encouraging best practices to keep enclaves "secure by default".

With Enclaver, running a secure enclave feels just like a `docker run`. More differences:

 - uses a Docker toolchain (build + store + sign + run)
 - routing and network connectivity work out of the box
 - network ingress/egress is protected through policy that is part of the enclave attestation
 - kernel used inside the enclave is versioned separately than the CLI tool (not true of `nitro-cli`)
 - automatically wrap KMS API calls with the enclave's attestation document with no effort on the user's part
 - kernel entropy is seeded and ready for operations

### How can I use Enclaver in my existing CI/CD tools?

If you produce a container for the service or part of your app you'd like to run with Enclaver, all you need to do is add an additional build step. First, check in an enclave configuration into your code with parameters for running the enclave and the desired network policy.

#### Continuous Integration (CI)

In your continuous integration (CI) tool, after your container build is complete, run `enclaver build` with a reference to the enclave configuration file. If desired, you can pass in the `from` container image via flag or environment variable to reference the container you just built instead of using the configuration file. The result of your build is another container image, the enclave image, which you can push to your registry.


#### Continuous Deployment (CD)

In your continuous deployment (CD) tool, update the references to the new enclave image. This is commonly in a systemd unit file and looks similar to `enclaver run registry.example.com/app:v1.0.1`. It might be useful to consider a systemd drop-in for this purpose. After the unit is updated, execute a `systemctl daemon-reload` to pick up the new unit file changes, and then `systemctl restart example.service` to restart the enclave. When the restart is triggered, Enclaver will read the signal to stop the existing enclave, pull down the new enclave image and then start it based on the parameters in the embedded configuration file.