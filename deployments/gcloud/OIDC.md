# Keyless GKE deploys from GitHub Actions

`deploy-dev.yaml`, `deploy-prod.yaml` and `redeploy-prod.yaml` sign in to Google Cloud with
Workload Identity Federation once the repository variable `GCP_WIF_PROVIDER` is set. Until then
they fall back to the long-lived JSON key in the `GKE_SA_KEY` secret. These are the one-time
steps that move them over, after which the key is deleted.

GitHub's OIDC token names the repository, the ref and the environment of each job. Google
accepts it only when those match, so a fork, another repository or a branch other than the
allowed one gets nothing.

| Who | Allowed when | Provider |
|:---|:---|:---|
| `deploy-dev.yaml` | repository `OyadotAI/oya-browser`, ref `refs/heads/main` | `github-dev` |
| `deploy-prod.yaml` | same repository, environment `production`, ref `refs/tags/v*` | `github-prod` |
| `redeploy-prod.yaml` | same repository, environment `production`, this workflow file | `github-prod` |

`redeploy-prod.yaml` is dispatched by hand from a branch, not a tag, so the prod provider also
accepts that one workflow; both prod paths still wait for an approver on the `production`
environment.

## 1. Fill in the values

The workflows read these from the `GKE_PROJECT`, `GKE_CLUSTER` and `GKE_ZONE` secrets and the
`GKE_SA_KEY` key's `client_email`. Use the same ones here.

```bash
PROJECT_ID=<GKE_PROJECT>                 # e.g. my-project
DEPLOY_SA=<deploy-sa-name>@${PROJECT_ID}.iam.gserviceaccount.com   # client_email in GKE_SA_KEY
REPO=OyadotAI/oya-browser
POOL=github
PROJECT_NUMBER=$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')
```

## 2. Create the pool and the two providers

```bash
gcloud services enable iamcredentials.googleapis.com sts.googleapis.com --project "$PROJECT_ID"

gcloud iam workload-identity-pools create "$POOL" \
  --project "$PROJECT_ID" --location global \
  --display-name "GitHub Actions"

gcloud iam workload-identity-pools providers create-oidc github-dev \
  --project "$PROJECT_ID" --location global --workload-identity-pool "$POOL" \
  --display-name "GitHub dev deploys" \
  --issuer-uri https://token.actions.githubusercontent.com \
  --attribute-mapping "google.subject=assertion.sub,attribute.repository=assertion.repository,attribute.ref=assertion.ref" \
  --attribute-condition "assertion.repository == '${REPO}' && assertion.ref == 'refs/heads/main'"

gcloud iam workload-identity-pools providers create-oidc github-prod \
  --project "$PROJECT_ID" --location global --workload-identity-pool "$POOL" \
  --display-name "GitHub prod deploys" \
  --issuer-uri https://token.actions.githubusercontent.com \
  --attribute-mapping "google.subject=assertion.sub,attribute.repository=assertion.repository,attribute.ref=assertion.ref,attribute.environment=assertion.environment" \
  --attribute-condition "assertion.repository == '${REPO}' && assertion.environment == 'production' && (assertion.ref.startsWith('refs/tags/v') || assertion.workflow_ref.startsWith('${REPO}/.github/workflows/redeploy-prod.yaml@'))"
```

## 3. Let the repository act as the deploy service account

```bash
gcloud iam service-accounts add-iam-policy-binding "$DEPLOY_SA" \
  --project "$PROJECT_ID" \
  --role roles/iam.workloadIdentityUser \
  --member "principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL}/attribute.repository/${REPO}"
```

The service account keeps the roles it has today (it already deploys with the JSON key). The
binding admits any token that passed a provider's condition, so the conditions above are the gate.
If prod should use a service account of its own, create one, give it the cluster role, and bind
it the same way with `attribute.environment/production` in place of `attribute.repository/${REPO}`.

## 4. Set the GitHub variables

Variables, not secrets: neither value is sensitive. The `production` environment's values
override the repository's for the jobs that run in it, so dev and prod pick different providers
under the same names.

```bash
DEV_PROVIDER=projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL}/providers/github-dev
PROD_PROVIDER=projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL}/providers/github-prod

gh variable set GCP_WIF_PROVIDER --repo "$REPO" --body "$DEV_PROVIDER"
gh variable set GCP_DEPLOY_SA    --repo "$REPO" --body "$DEPLOY_SA"
gh variable set GCP_WIF_PROVIDER --repo "$REPO" --env production --body "$PROD_PROVIDER"
gh variable set GCP_DEPLOY_SA    --repo "$REPO" --env production --body "$DEPLOY_SA"
```

Run a dev deploy (push to `main`) and a prod redeploy from the Actions tab, and check the auth
step that ran is the one with `workload_identity_provider`.

## 5. Delete the JSON key

Only after both deploys above went through without it.

```bash
gcloud iam service-accounts keys list --iam-account "$DEPLOY_SA" --managed-by user
gcloud iam service-accounts keys delete <KEY_ID> --iam-account "$DEPLOY_SA"   # the private_key_id in GKE_SA_KEY
gh secret delete GKE_SA_KEY --repo "$REPO"
gh secret delete GKE_SA_KEY --repo "$REPO" --env production 2>/dev/null || true
```

To stop the fallback from ever coming back, an org policy can forbid new keys on the project:
`gcloud resource-manager org-policies enable-enforce iam.disableServiceAccountKeyCreation --project "$PROJECT_ID"`.
