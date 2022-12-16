# --- BUILD STAGE
FROM rust:alpine as builder

RUN apk update && \
    apk upgrade

RUN apk add --no-cache build-base openssl-dev wget unzip

WORKDIR /usr/src/hpcac_toolkit

ARG TERRAFORM_VERSION=1.3.6
RUN wget https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip
RUN unzip terraform_${TERRAFORM_VERSION}_linux_amd64.zip

ADD . .

RUN cargo build --release


# --- RUNTIME STAGE
FROM alpine:3.17 as runtime

COPY --from=builder /usr/src/hpcac_toolkit/terraform /usr/bin/terraform
COPY --from=builder /usr/src/hpcac_toolkit/target/release/hpcac-cli /usr/local/bin/hpcac-cli

CMD ["/bin/sh"]
