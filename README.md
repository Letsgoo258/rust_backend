# ERAVAYA ERP

A full-stack multi-tenant school ERP.
- **Backend:** Rust, Axum, SQLx (PostgreSQL)
- **Frontend:** Next.js, Tailwind CSS
- **Infrastructure:** Docker, Render (Planned)

See the `docs/` folder for complete technical architecture and schema details.

# Start in Development Mode (Live reload):
docker compose -f docker-compose.dev.yml up

# Start in Production Mode:
docker compose up --build -d
