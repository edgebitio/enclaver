.PHONY: enclaver demo-app enclave-image run-enclave

enclaver:
	go install ./cmd/enclaver/

demo-app:
	docker build . -t example:latest -f example/Dockerfile

enclave-image:
	enclaver build -f example/policy.yaml

run-enclave:
	docker run --net=host --privileged example-enclave
