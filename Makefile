.PHONY: help wizard migrate server server-dev browser browser-dev browser-build browser-dist-mac browser-dist-linux ui ui-dev ui-build deploy-dev deploy-prod release logs-dev logs-prod pods-dev pods-prod restart-dev restart-prod k8s-dev k8s-prod docker-build docker-run docker-up docker-down docker-browser docker-scale

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Server ──

server: ## Start server
	cd server && npm start

server-dev: ## Start server with auto-reload
	cd server && npm run dev

server-install: ## Install server dependencies
	cd server && npm install

# ── UI (Next.js) ──

ui: ## Start UI (production)
	cd ui && npm run build && npm start

ui-dev: ## Start UI dev server
	cd ui && npm run dev

ui-build: ## Build UI for production
	cd ui && npm run build

ui-install: ## Install UI dependencies
	cd ui && npm install

# ── Browser App ──

browser: ## Run browser app
	cd browser && npm start

browser-dev: ## Run browser app (with stderr)
	cd browser && npm run dev

browser-install: ## Install browser dependencies
	cd browser && npm install

browser-build: ## Build browser for current platform
	cd browser && npm run dist

browser-dist-mac: ## Build macOS DMG (universal)
	cd browser && npm run dist:mac

browser-dist-linux: ## Build Linux AppImage
	cd browser && npm run dist:linux

# ── Docker ──

docker-build: ## Build server Docker image
	docker build -t oya-browser-server .

docker-run: ## Run server in Docker
	docker run --rm -p 3100:3100 -e API_KEYS=$${API_KEYS:-test} oya-browser-server

docker-up: ## Start server via docker compose
	docker compose up -d

docker-down: ## Stop docker compose
	docker compose down

docker-browser: ## Build browser Docker image
	docker build -t oya-browser browser

docker-scale: ## Scale browser instances (usage: make docker-scale N=5)
	docker compose up -d --scale browser=$(N)

# ── Deploy ──

deploy-dev: ## Push to main (triggers dev deploy)
	git push origin main

deploy-prod: release ## Tag and push (triggers prod deploy)

release: ## Full release: build the desktop app, tag, push, publish npm SDK + CLI (V=1.2.0 to pin the version)
	@./k8s/scripts/release.sh $(V)

release-no-desktop: ## Release without rebuilding the desktop app — the last one is carried forward
	@./k8s/scripts/release.sh --no-desktop $(V)

# ── Kubernetes ──

pods-dev: ## Show dev pods
	kubectl get pods -n oya-browser-dev

pods-prod: ## Show prod pods
	kubectl get pods -n oya-browser

logs-dev: ## Tail dev server logs
	kubectl logs -f deployment/server -n oya-browser-dev

logs-prod: ## Tail prod server logs
	kubectl logs -f deployment/server -n oya-browser

restart-dev: ## Restart dev pods
	kubectl rollout restart deployment/server -n oya-browser-dev

restart-prod: ## Restart prod pods
	kubectl rollout restart deployment/server -n oya-browser

k8s-dev: ## Apply dev k8s manifests
	kubectl apply -k k8s/overlays/dev

k8s-prod: ## Apply prod k8s manifests
	kubectl apply -k k8s/overlays/prod

status-dev: ## Full dev status
	kubectl get pods,svc,ingress,hpa -n oya-browser-dev

status-prod: ## Full prod status
	kubectl get pods,svc,ingress,hpa -n oya-browser

# ── Setup ──

install: server-install ui-install browser-install ## Install all dependencies

setup: install ## First-time setup
	cd server && cp -n .env.example .env 2>/dev/null || true
	cd ui && cp -n .env.example .env.local 2>/dev/null || true
	@echo "\n✓ Done. Edit server/.env and ui/.env.local then run: npm run dev"

wizard: ## Interactive self-host installer (database, browsers, LLM)
	@npm run build --workspace=@oya-ai/cli --silent
	@node packages/cli/dist/index.js install $(ARGS)

migrate: ## Apply SQL migrations (needs DATABASE_URL; SQLite needs none)
	@node server/migrations/run.mjs $(ARGS)

dev: ## Start API and Next.js on http://localhost:3100
	npm run dev
