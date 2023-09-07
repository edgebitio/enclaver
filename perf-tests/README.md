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

## Test setup

Install the Apache HTTP server benchmarking tool (`ab`):

```console
$ sudo yum install --assumeyes httpd-tools
```

## Running the tests

[`run_enclave.sh`](run_enclave.sh) contains the commands to start the
nitro enclave. The first argument specifies the number of vCPUs to use
for the enclave (defaults to `4`).

After starting the enclave, run `ab` to benchmark the
performance. This can be done in one line (with a delay to allow the
enclave to start):

```console
$ sudo ./run_enclave.sh && sleep 5; ab -n 10000 -c 100 localhost:8082/busy
```

## Sample results

The following test results were gathered on a c6a.2xlarge instance.

With four vCPUs:

```console
$ sudo ./run_enclave.sh 4 && sleep 5; ab -n 10000 -c 100 localhost:8082/busy
...
Concurrency Level:      100
Time taken for tests:   2.841 seconds
Complete requests:      10000
Failed requests:        0
Total transferred:      1470000 bytes
HTML transferred:       300000 bytes
Requests per second:    3520.03 [#/sec] (mean)
Time per request:       28.409 [ms] (mean)
Time per request:       0.284 [ms] (mean, across all concurrent requests)
Transfer rate:          505.32 [Kbytes/sec] received

Connection Times (ms)
              min  mean[+/-sd] median   max
Connect:        0    1   0.8      0      11
Processing:     1   28  24.6     24     292
Waiting:        1   27  24.6     23     291
Total:          1   28  24.7     24     293
```

With two vCPUs:

```console
$ sudo ./run_enclave.sh 2 && sleep 5; ab -n 10000 -c 100 localhost:8082/busy
...
Concurrency Level:      100
Time taken for tests:   5.743 seconds
Complete requests:      10000
Failed requests:        0
Total transferred:      1470000 bytes
HTML transferred:       300000 bytes
Requests per second:    1741.36 [#/sec] (mean)
Time per request:       57.426 [ms] (mean)
Time per request:       0.574 [ms] (mean, across all concurrent requests)
Transfer rate:          249.98 [Kbytes/sec] received

Connection Times (ms)
              min  mean[+/-sd] median   max
Connect:        0    0   0.2      0       2
Processing:     1   57  30.8     54     334
Waiting:        1   57  30.6     54     334
Total:          1   57  30.9     54     335
```
