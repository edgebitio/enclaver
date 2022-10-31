---
title: "Running Hashicorp Vault in an Enclave"
layout: docs-enclaver-single
category: guides
---

# Running Hashicorp Vault in an Enclave

Enclaver works by transforming a containerized application into a new container image which runs the underlying application in an enclave.

To demonstrate how this works, let’s run HashiCorp Vault in an enclave. Vault has good security out of the box, but if we trust our attestation, we can cryptographically guarantee that our Vault will only be unsealed in a known trusted state and all material can’t be leaked outside the enclave.

In addition, we will make use of Vault's auto-unsealing capability using a KMS key to guarantee that only the genuine Vault image running in an enclave can perform the unseal.

In our build step, you will see that some "measurements" are displayed. These are a [cryptographic attestation][attestation], an identity for the code, that can't be spoofed or stolen by an attacker. Our key policy will contain some of these values, which prove that only _this specific code_ can access the unseal key.

For this example you’ll need an EC2 instance with support for Nitro Enclaves enabled (`c6a.xlarge` is the cheapest x86 instance) and Docker installed.  See the [Deploying on AWS][deploy-aws] for more details.

[![CloudFormation](img/launch-stack-x86.svg)][cloudformation-x86]

[cloudformation-x86]: https://us-east-1.console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/create/review?templateURL=https://enclaver-cloudformation.s3.amazonaws.com/enclaver.cloudformation-x86.yaml&stackName=Enclaver-Demo
[cloudformation-arm]: https://us-east-1.console.aws.amazon.com/cloudformation/home?region=us-east-1#/stacks/create/review?templateURL=https://enclaver-cloudformation.s3.amazonaws.com/enclaver.cloudformation-arm.yaml&stackName=Enclaver-Demo

## Clone the Git repo

After you've launched the CloudFormation stack, ssh onto the EC2 instance and clone the following git repo to get the files we'll be using in this guide:

```sh
$ sudo yum install git
$ git clone https://github.com/edgebitio/vault.git
$ cd vault
```

## Launch Consul

In this guide, we'll be using Consul as the storage backend for Vault. Since Vault encrypts all the data prior to storing it in Consul, we don't have to worry about running Consul in an enclave.

If you don't have a Consul instance running, it's easy to start one using Docker. This command will run a Consul cluster of size one and the `DATA_DIR` environment variable is set to a directory for Consul to store its data.

```sh
$ docker run --rm -d --net=host \
    --name consul \
    -v $DATA_DIR:/consul/data \
    consul:1.13 \
    agent -server \
          -bind 127.0.0.1 \
          -client="0.0.0.0" \
          -bootstrap-expect 1
```

## Create a new AWS KMS key

We will use Enclaver's KMS integration to ensure that only Vault running in our enclave can decrypt two secrets:

1. The TLS private key used to secure communcation to our Vault instance
1. The unseal key used to decrypt our Vault secrets stored in Consul

To get started, create a new symmetric KMS key which we will use to encrypt the two secrets above. Ensure that the IAM role created by CloudFormation can use the key.

<details>
  <summary>View step by step instructions for key creation</summary>

- Start by going to [AWS KMS Console](https://console.aws.amazon.com/kms/home) and clicking the "Create Key" button.

- Make sure the "Key type" is selected as "Symmetric" and "Key Usage" is set to "Encrypt and decrypt". Click "Next".

- Enter an alias, e.g. "vault-key" and add a description and tags, if you'd like. Click "Next".

- Select your username as the key administrator and click "Next".

- Select your username and the `Enclaver-Demo-DemoIAMRole-XXXX` role to define the key usage permisssions. Click "Next".

- Review the configuration and policy and click "Finish".

</details>

Once the key is created, copy the KeyId (it's a UUID) and set `AWS_KEY_ID` environment variable to that value.

```sh
$ export AWS_KEY_ID=822bd842-ad07-4ca2-b8af-97fcd13fa670
```

Also configure the region where your key is stored:

```sh
$ export AWS_DEFAULT_REGION="us-east-1"
```

## Generate a TLS certificate for Vault

In this guide, we'll generate a self-signed TLS certificate for Vault to use. If you already use an exisiting certificate, you can skip the generation step and encrypt your existing private key.

The `generate-cert.sh` script first generates a certificate for the provided hostname, eg `vault.local` and then encrypts the corresponding private key using the KMS key created above.

```sh
$ ./generate-cert.sh "vault.local" $AWS_KEY_ID
```

The result should be two files: `cert.pem` and `key.pem.enc`. Note that `localhost` and `127.0.0.1` are added to the SAN list in addition to the desired domain name.

## Build the Vault Docker image

We'll make use of the official Vault Docker image but will layer in the bits to decrypt the TLS private key prior to launch.

The `Dockerfile` installs the AWS CLI, copies in the certificate and the key files, then installs a new entrypoint which performs the decryption before passing control to the original entrypoint.

```sh
$ docker build --build-arg AWS_KEY_ID --build-arg AWS_DEFAULT_REGION -t vault:enclave-src .
```

At this point `vault:enclave-src` contains the source image for Enclaver to build into an enclave image.

## Review the Manifest

Enclaver uses a declarative [manifest][manifest] file to define the contents of an enclave. The `enclaver.yaml` that you cloned looks like this and doesn't need to be modified:

```yaml
version: v1
name: "enclaver-vault"
sources:
  app: "vault:enclave-src"
target: "vault:enclave"
ingress:
  - listen_port: 8200
egress:
  allow:
    - 169.254.169.254
    - kms.*.amazonaws.com
    - host
kms_proxy:
  listen_port: 9999
defaults:
  memory_mb: 3000
```

It specifies that it will use `vault:enclave-src` that we built in the previous step as its source image.

The resulting Nitro Enclave image (EIF) together with the supporting tooling will be packaged into the `vault:enclave` target Docker image.

Since Vault has been configured to listen on port 8200 for client connections, we need to specify that in the manifest as well.

Next, we enable egress traffic by configuring a list of allowed addresses.

- `169.254.169.254` is the well-known IP of the AWS Instance Metadata Service and will be used to acquire the AWS credentials.
- `kms.*.amazonaws.com` allows the enclave to talk to KMS in any region.
- `host` is a special hostname to refer to the parent EC2 instance and will allow Vault to reach Consul.

Enclaver includes a KMS proxy to attach an attestation document to select KMS actions.
This allows an application, such as Vault, to take advantage of the attestation features of KMS without modification.

## Build the Enclave Image

Now, we’ll ask Enclaver to build a new container image using this policy:

```sh
$ enclaver build -f manifest.yaml
```

Once completed, it will output part of the [attestation][attestation] of the image, which are hashes ("measurements") that look like:

```sh
EIF Info: EIFInfo {
    measurements: EIFMeasurements {
        pcr0: "bbd4eed4d2e87687ed2c802d49002f42b1ce7a3ee376252415fccf7460267b5ef60da3d651f940b20bd36364d9329a27",
        pcr1: "bcdf05fefccaa8e55bf2c8d6dee9e79bbff31e34bf28a99aa19e6b29c37ee80b214a414b7607236edf26fcb78654e63f",
        pcr2: "51bf425a88cb47112fd742f6519f8754cce2f94cc37d40f69544efa0528b250168731241033561511eb9b40c1de0003c",
    },
}
```

To review, the Enclaver build process did the following:

1. Built a new Docker image, based on the source `vault` image and injected several Enclaver-specific components:
    - the Enclaver internal supervisor + proxy binary
    - the policy file we provided
2. Converted this Docker image into a Nitro Enclaves compatible EIF file
3. Built a new container image which bundles the Enclaver “external proxy” and Nitro Enclave launcher.

If you are curious about these components, read about the [architecture][architecture].

## Update the KMS key policy

The CloudFormation created a "DemoIAMRole" role that it associated with the EC2 instance and which the enclave will inherit. Our goal is to allow the role to use the KMS key excluding decryption. 

The decryption operation will have an additional constraint – restrict decryption to enclaves running a specific image hash.

The `pcr0` output during the build contains the desired hash of the enclave image, which will give identity to the enclave running Vault.

Let's modify the policy to only allow the Vault enclave with the matching hash to decrypt the key.

1. In the [AWS KMS Console](https://console.aws.amazon.com/kms/home) select the key and then click the "Switch to policy view" button.

2. Click the "Edit" button.

3. Locate the "Allow use of the key" statement and remove the `kms:::Decrypt` action so the statement looks as follows:

    ```
    	{
    		"Sid": "Allow use of the key",
    		"Effect": "Allow",
    		"Principal": {
    			"AWS": [
    				"arn:aws:iam::077296892761:user/eyakubovich",
    				"arn:aws:iam::077296892761:role/Enclaver-Demo-DemoIAMRole-175IWIYZRKO8F"
    			]
    		},
    		"Action": [
    			"kms:Encrypt",
    			"kms:ReEncrypt*",
    			"kms:GenerateDataKey*",
    			"kms:DescribeKey"
    		],
    		"Resource": "*"
    	},
    ```

4. Add another statement that grants the `Decrypt` ability soley to the Vault image running inside the enclave:

    ```
    	{
    		"Sid": "Allow decryption by Vault only",
    		"Effect": "Allow",
    		"Principal": {
    			"AWS": "arn:aws:iam::077296892761:role/Enclaver-Demo-DemoIAMRole-175IWIYZRKO8F"
    		},
    		"Action": "kms:Decrypt",
    		"Resource": "*",
    		"Condition": {
    			"StringEqualsIgnoreCase": {
    				"kms:RecipientAttestation:PCR0": "bbd4eed4d2e87687ed2c802d49002f42b1ce7a3ee376252415fccf7460267b5ef60da3d651f940b20bd36364d9329a27"
    			}
    		}

    	},
    ```

Be sure to put in the PCR0 value that `enclaver build` printed at the end.

Finally, click the "Save changes" button.

## Run the Enclave

The result of the build step was a new Docker image named `vault:enclave`. Running this image starts up Vault in a Nitro Enclave on your AWS machine!

On your EC2 machine, start the enclave with Enclaver:

```sh
$ docker run --rm -it \
  --net=host \
  --name=vault \
  --device=/dev/nitro_enclaves \
  vault:enclave
```

It's normal to see this failure towards the end, because we haven't initialized Vault yet.

```
failed to unseal core: error="stored unseal keys are supported, but none were found"
```

*Note:* This used the interactive (vs detached) mode to run the container to easily see the logs. Once you've confirmed that everything look OK, you can restart the container in the detached (`-d`) mode.

## Initialize the Vault

Before the Vault can be used, it needs to be initialized. You can use the official Docker Vault image to run the CLI command.

Since the Vault client needs to talk TLS with the Vault server, we need to be sure to mount the directory that contains the certificate.

```sh
$ docker run --rm -it --net=host \
  -v $(pwd):/opt/ \
  -e VAULT_ADDR=https://127.0.0.1:8200 \
  -e VAULT_CACERT=/opt/cert.pem \
  vault:latest operator init
```

The output will contain the recovery keys and the initial root token. Save the token to the environment variable and create an alias to run Vault commands:

```sh
$ export VAULT_TOKEN=hvs.AGVp3Rc9AShUXGf3iJVnM2JP
$ alias vault-cli="docker run --rm -it --net=host -v $(pwd):/opt/ -e VAULT_ADDR=https://127.0.0.1:8200 -e VAULT_CACERT=/opt/cert.pem -e VAULT_TOKEN=$VAULT_TOKEN vault:latest"
```

Confirm that the vault has been initialized and unsealed:

```sh
$ vault-cli vault:latest status
Key                      Value
---                      -----
Recovery Seal Type       shamir
Initialized              true
Sealed                   false
Total Recovery Shares    5
Threshold                3
Version                  1.12.0
Build Date               2022-10-10T18:14:33Z
Storage Type             consul
Cluster Name             vault-cluster-287b7bd6
Cluster ID               da4e7a72-4d61-45c2-e2e4-0800265675d0
HA Enabled               true
HA Cluster               https://127.0.0.1:8201
HA Mode                  active
Active Since             2022-10-29T16:20:04.604490214Z
```

You should be able to restart the Vault container now and use the status command to confirm that it starts in the unsealed state.

## Using Vault

Let's use the newly running Vault to store some key/value secrets.

```sh
# create secret
$ vault-cli vault secrets enable -path=secret/ kv-v2
$ echo 'path "secret/data/*" {
    capabilities = ["create", "update", "read"]
}' > policy.hcl
# create policy
$ vault-cli vault policy write my-policy /opt/policy.hcl
# save secret values
$ vault-cli kv put -mount=secret creds username=john password=supersecret
```

Let's review what we accomplished. We have an instance of Vault that can automatically unseal itself, but _only_ from our trusted enclave image. It's impossible to introspect or reconfigure the environment inside the enclave and there is no shell access to our enclave.

## Next Steps

This example walked through running an entire application in an enclave. Next, experiement with running a specific microservice or a security-centric function within an enclave.

Check out the [example Python app](guide-app.md) for a walkthrough.

[deploy-aws]: deploy-aws.md
[manifest]: manifest.md
[dockerfile]: TODO-add-me
[attestation]: architecture.md#calculating-cryptographic-attestations
[unit]: deploy-aws.md#run-via-systemd-unit
[guide-app]: guide-app.md
[architecture]: architecture.md