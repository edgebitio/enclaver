---
title: "Using KMS Proxy"
layout: docs-enclaver-single
category: guides
---

# KMS Proxy Overview

To derive the most value from Enclaver, your application's sensitive data should be encrypted using a key stored in AWS Key Management Service (KMS) to be decrypted only within an enclave.
To ensure the data can only be decrypted from within an enclave running a specific image hash, a KMS key policy can constrain the Decrypt operation to requests with given PCR values (hashes).
See [Using cryptographic attestation with AWS KMS](https://docs.aws.amazon.com/enclaves/latest/user/kms.html) for details.

# How the Attestation Document Works
The PCR values are included in an attestation document, signed by AWS. An application running inside an enclave can retrieve such a document and attach it to the KMS requests.
However it is a cumbersome process to be done directly by the application. The Enclaver includes a KMS proxy that makes this process transparent.
By configuring the application to connect to the KMS proxy running on `localhost`, all outgoing KMS requests that support it (e.g. Decrypt) will have the attestation documents attached to them.

# Enabling the KMS Proxy

Enable the KMS proxy in your `enclaver.yaml` as follows:

```yaml
kms_proxy:
  listen_port: 9999

egress:
  allow:
    - 169.254.169.254
    - kms.*.amazonaws.com
```

When the enclave starts up, Enclaver will define a `AWS_KMS_ENDPOINT=http://127.0.0.1:9999` environment variable. The value can be passed into the AWS SDK to override the default endpoint.
The exact details are language specfiic. See below for examples of the most popular languages.

*WARNING*: Do not expose the KMS proxy port (9999 in this example) in the `ingress` section. Doing so will expose the KMS proxy outside the enclave, allowing untrusted code to decrypt the data.

# Configuring AWS SDK with KMS Proxy Endpoint

The following examples show how to pass the environment variable to the AWS SDK in the most idiomatic way per programming language.

## Python
```python
import os, boto3

kms = boto3.client('kms', endpoint_url=os.environ['AWS_KMS_ENDPOINT'])
resp = kms.decrypt(CiphertextBlob=b'...ciphertext...')
plaintext = resp['Plaintext']
```

## Ruby

```ruby
require "base64"
require 'aws-sdk'

client = Aws::KMS::Client.new(
  endpoint: ENV["AWS_KMS_ENDPOINT"]
)

resp = client.decrypt({
  ciphertext_blob: ciphertext
})

plaintext = resp.plaintext
```

## NodeJS

Node AWS SDK does not automatically honor `http_proxy` and `no_proxy` environment variables and require a `global-agent` module to inject HTTP proxy support.

```js
import { KMSClient, DecryptCommand } from "@aws-sdk/client-kms";
import { bootstrap } from 'global-agent';
bootstrap();

global.GLOBAL_AGENT.HTTP_PROXY = process.env.http_proxy;
global.GLOBAL_AGENT.NO_PROXY = process.env.no_proxy;

const kms = new KMSClient({
  endpoint: process.env.AWS_KMS_ENDPOINT
});

const decrypt = const response = await kms.send(new DecryptCommand({
    CiphertextBlob: ciphertext
}));

var plaintext = response.Plaintext);
```

## Go

```golang
endpoint := os.Getenv("AWS_KMS_ENDPOINT")
customResolver := aws.EndpointResolverWithOptionsFunc(func(service, region string, options ...interface{}) (aws.Endpoint, error) {
	return aws.Endpoint{
		PartitionID:   "aws",
		URL:           endpoint,
		SigningRegion: region,
	}, nil
})

cfg, err := config.LoadDefaultConfig(context.TODO(), config.WithEndpointResolverWithOptions(customResolver))

client := kms.NewFromConfig(cfg)

resp, err := client.Decrypt(context.TODO(), &kms.DecryptInput{
	CiphertextBlob: []byte(ciphertext),
})

plaintext = resp.Plaintext

```

# See It in Action

For a sample Python application using the KMS proxy, checkout the [Running a Python App](guide-app.md) guide.
