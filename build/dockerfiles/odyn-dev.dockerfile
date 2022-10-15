FROM scratch

COPY ./odyn /usr/local/bin/odyn

ENTRYPOINT ["/usr/local/bin/odyn"]