## What is in this directory?

Several tests have been done to evaluate performance of
enclaver/nitro-enclaves. As part of these tests, a simple Go server
runs within the enclave and burns some CPU every time the endpoint
(`http://localhost:8082/busy`) is fetched.

## Instance Setup

1. Provision a new nitro instance (`c6a.2xlarge` or `c6g.xlarge` is
   recommended) and follow the [first-enclave guide][first-enclave] to
   setup up the nitro_allocator.yaml file.

2. Modify the file `/etc/nitro_enclaves/allocator.yaml`, setting
   `cpu_count` to `4`:

   ```console
   $ sudo sed --in-place 's/cpu_count: 2/cpu_count: 4/g' /etc/nitro_enclaves/allocator.yaml
   ```

3. Restart the nitro-allocator service:

   ```console
   $ sudo systemctl restart nitro-enclaves-allocator.service
   ```

[first-enclave]: ../docs/guide-first.md

## JMeter setup

SSH into your machine and run the following commands

```console
$ mkdir jmeter
$ cd jmeter
$ wget --quiet https://archive.apache.org/dist/jmeter/binaries/apache-jmeter-5.5.tgz
$ tar --extract --file apache-jmeter-5.5.tgz
$ sudo yum install --assumeyes java-11-amazon-corretto-headless
```

## Running the tests

[`run_enclave.sh`](run_enclave.sh) contains the commands to start the
nitro enclave. The first argument specifies the number of vCPUs to use
for the enclave (defaults to `4`). [`run_jmeter.sh`](run_jmeter.sh)
runs the JMeter test suite and prints the summary of the results.

The two scripts can be invoked in one line:

```console
$ sudo ./run_enclave.sh && ./run_jmeter.sh
```

## Sample results

With four vCPUs:
```
summary = 12000 in 00:00:42 = 283.7/s Avg: 155 Min: 12 Max: 2707 Err: 0 (0.00%)
```

With two vCPUs:
```
summary = 12000 in 00:00:41 = 290.3/s Avg: 108 Min: 14 Max: 1755 Err: 0 (0.00%)
```

At the moment, there is no significant difference in TPS by increasing
the number of vCPUs.
