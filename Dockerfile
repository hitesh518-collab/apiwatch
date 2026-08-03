FROM alpine:3.21
COPY apiwatch /usr/local/bin/apiwatch
ENTRYPOINT ["apiwatch"]
