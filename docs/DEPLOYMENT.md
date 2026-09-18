# Deployment Documentation

## Docker
A multi-stage Dockerfile is provided.
`docker build -t eravaya-api .`

## Environment
Requires `DATABASE_URL`, `APP_PORT`, `SESSION_SECRET`, etc.

## Migrations
Run SQLx migrations prior to deployment to ensure the schema is up-to-date.
