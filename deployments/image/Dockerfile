# syntax=docker/dockerfile:1.7
#
# The image every deployment in deployments/ runs: the Oya server (built from
# the repository's root Dockerfile, passed in as BASE_IMAGE) plus what a
# production control plane needs at start:
#
#   - psql and the SQL migrations, so it applies its own schema on start
#   - kubectl, for launching browsers as pods (deployments/k8s, deployments/gcloud)
#   - the Amazon RDS certificate bundle, so Postgres TLS is verified on ECS
#
# Built by deployments/lib.sh; build context is the repository root.
ARG BASE_IMAGE
FROM ${BASE_IMAGE}

RUN apk add --no-cache postgresql-client kubectl

COPY server/migrations/ ./migrations/
ADD --chmod=644 https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem /etc/ssl/certs/rds-global-bundle.pem
COPY --chmod=755 deployments/image/entrypoint.sh /usr/local/bin/oya-entrypoint

CMD ["oya-entrypoint"]
