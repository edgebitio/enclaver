---
title: "Deploy on Kubernetes"
layout: docs-enclaver-single
category: deploying
---

# Deploy on Kubernetes running on AWS

Enclaver can be used with Kubernetes to run Nitro Enclaves on qualified Nodes in your EKS, Rancher/k3s or OpenShift cluster. Users of your cluster can use an enclave image (from `enclaver build`) inside of a Deployment.

## Running an Enclave

Running an Deployment that uses an enclave is very easy. Here's the easiest example:

TODO: this needs further testing
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: example-enclave
  namespace: default
spec:
  replicas: 1
  selector:
    matchLabels:
      app: example
  template:
    metadata:
      labels:
        app: example
    spec:
      topologySpreadConstraints:
      - maxSkew: 1
        topologyKey: kubernetes.io/hostname
        whenUnsatisfiable: DoNotSchedule
      nodeSelector:
        edgebit.io/enclave: nitro
      containers:
      - name: webapp 
        image: registry.example.com/webapp:latest
        ports: 
           - containerPort: 80
             name: web
      - name: enclave 
        image: us-docker.pkg.dev/edgebit-containers/containers/no-fly-list:enclave-latest
        ports: 
           - containerPort: 8001
             name: enclave-app
        volumeMounts:
        - mountPath: /dev/nitro_enclaves
          name: nitro_enclaves
      restartPolicy: Always
      volumes:
      - hostPath:
          path: /dev/nitro_enclaves
        name: nitro_enclaves
```

There are two parts to call out:
1. The `nodeSelector` is selecting only our Nitro enabled Nodes and the `topologySpreadConstraints` ensures that each Node only runs one enclave at a time. See below for more robust hardware tracking.
2. The `args` of the enclave container points to your enclave image.

This assumes you have Nodes available that can run Nitro Enclaves.

## Add Qualified Nodes to your Cluster

Only certain [EC2 instance types][instance-req] can run Nitro Enclaves. `c6a.xlarge` is the cheapest qualifying instance type as of this writing) and Docker installed.  See [the Deploying on AWS](deploy-aws.md) for more details.

Due to Amazon restrictions, each EC2 machine can only run a single enclave at a time. This is enforced by teaching the scheduler about the Nitro Enclave hardware resource below.

### Label Nodes

Label your Nodes with `edgebit.io/enclave=nitro` so that your Deployment can target the qualified Nodes.

### Tainting is Optional

You may also Taint your Nodes so other workloads don't land on it, but in most cases we don't think that is useful. Enclaves work well when deployed like a sidecar, either directly in a Pod or with affinity to another Deployment. The larger instances are also more expensive, so you'll probably want to use those resources to run other Pods unless your security posture won't allow it.

## Testing the Enclave

The example app answers web requests on port 443. You can make a Service and Load Balancer to address all of the Pods, or for a simple test, port-forward to the Pod:

TODO: update ports once final logic is in place
```sh
$ kubectl port-forward <podname> 8001:8001
```

Then send a request to the forward port, which will be answered from within the enclave:

```sh
$ curl localhost:8001
"https://edgebit.io/enclaver/docs/0.x/guide-app/"
```

Jump over to the [simple Python app][app] guide (the output URL above) that explains our sample application in more detail.

## Extend Scheduler with Nitro Enclaves Hardware Resource

There are a few options for extending the Kubernetes scheduler to understand Nitro Enclaves. The goal is to ensure that only one enclave is running on each Node at a time, which is a limitation from Amazon.

### Using Smarter Device Manager

ARM's [Smarter Device Manager][device-manager] is designed to track special hardware resources so the Kubernetes scheduler can manage them correctly. You can [read more][eks-blog] or install it pre-configured with the Node labels from above:

```sh
$ kubectl create -f smarter-device-manager-ds-with-cm.yaml
```

### OpenShift's Node Feature Discovery Operator

OpenShift users will want to use the [Node Feature Discovery Operator][nfd] from Red Hat. This will install it from OperatorHub on your cluster:

TODO: write this file
```
$ kubectl create -f node-feature-discovery-install.yaml
```

After installation, target the Nitro hardware device with this config:

TODO: target the nitro devices correctly

```yaml
apiVersion: nfd.openshift.io/v1
kind: NodeFeatureDiscovery
metadata:
  name: nfd-instance
  namespace: openshift-nfd
spec:
  instance: "" # instance is empty by default
  topologyupdater: false # False by default
  operand:
    image: registry.redhat.io/openshift4/ose-node-feature-discovery:v4.10
    imagePullPolicy: Always
  workerConfig:
    configData: |
        sources:
          pci:
            deviceClassWhitelist: ["0200", "03"]
```

## Troubleshooting

TODO: add troubleshooting

[device-manager]: https://gitlab.com/arm-research/smarter/smarter-device-manager
[eks-blog]: https://github.com/spkane/aws-nitro-cli-for-k8s/blob/d3e318f8de2690bc5507e50f0cdbe6be98dd9717/k8s/smarter-device-manager-ds-with-cm.yaml
[instance-req]: https://docs.aws.amazon.com/enclaves/latest/user/nitro-enclave.html#nitro-enclave-reqs
[nfd]: https://docs.openshift.com/container-platform/4.10/hardware_enablement/psap-node-feature-discovery-operator.html