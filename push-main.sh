# push-main.sh
#!/usr/bin/env bash
set -e

MSG="${1:?Informe a mensagem de commit. Ex.: ./push-main.sh 'feat(header): separar BaseHeader/AppHeader sem mudar visual'}"

git checkout main
git add -A
git commit -m "$MSG" || true
git pull --rebase origin main
git push origin main
