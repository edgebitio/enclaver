FROM edgebitio/nitro-cli:latest

COPY ./enclaver-run /usr/local/bin/enclaver-run

ENTRYPOINT ["/usr/local/bin/enclaver-run"]