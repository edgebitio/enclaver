## What is in this folder?

We have done several runs to evaluate performace of enclaver/nitro-enclaves. As part of these tests I have created a simple GO server that works using a self signed SSL certificate and does a naive crypto operation.

API used to test - https://localhost:8082/hello

## Instance Setup

1. Provision a new nitro instance and follow the steps here to setup up nitro_allocator.yaml file https://edgebit.io/enclaver/docs/0.x/guide-first/

2. Modify the file `/etc/nitro_enclaves/allocator.yaml` to `cpu_count: 4`

3. Restart the nitro-allocator service using `sudo systemctl restart nitro-enclaves-allocator.service`

## JMeter setup

SSH into your machine and run the following commands

```
mkdir jmeter
cd jmeter
wget https://archive.apache.org/dist/jmeter/binaries/apache-jmeter-5.5.tgz
tar -xf apache-jmeter-5.5.tgz
sudo amazon-linux-extras install java-openjdk11
```

## Running the tests

The script `./run_in_enclave.sh` contains the commands to start the nitro enclave.

Comment out the 4vcpu part of the script if you are using 2cvpu to start enclaves and vice-versa.

1. Start the enclave using the command `./run_in_enclave.sh`
2. Run JMeter tests using `./run_jmeter_test.sh`


## Sample results 

With 4vCPUs - 
`summary =  12000 in 00:00:42 =  283.7/s Avg:   155 Min:    12 Max:  2707 Err:     0 (0.00%)`

With 2vCPUs - `summary =  12000 in 00:00:41 =  290.3/s Avg:   108 Min:    14 Max:  1755 Err:     0 (0.00%)`

As you can see there is no significant difference in TPS by increasing the number of vCPUs.
 