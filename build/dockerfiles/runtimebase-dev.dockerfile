FROM edgebitio/nitro-cli:latest

COPY ./enclaver /usr/local/bin/enclaver

ENTRYPOINT ["/usr/local/bin/enclaver", "run-eif", "--eif-file", "/enclave/application.eif"]