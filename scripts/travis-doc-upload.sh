#!/bin/sh

PROJECT_NAME=html5ever
DOCS_REPO=kmcallister/docs.git
SSH_KEY_TRAVIS_ID=acec625b8c88

set -e

[ "$TRAVIS_BRANCH" = master ]

[ "$TRAVIS_PULL_REQUEST" = false ]

echo "<meta http-equiv=refresh content=0;url=`echo $TRAVIS_REPO_SLUG | cut -d '/' -f 2`/index.html>" > target/doc/index.html

eval key=\$encrypted_${SSH_KEY_TRAVIS_ID}_key
eval iv=\$encrypted_${SSH_KEY_TRAVIS_ID}_iv

mkdir -p ~/.ssh
openssl aes-256-cbc -K $key -iv $iv -in scripts/id_rsa.enc -out ~/.ssh/id_rsa -d
chmod 600 ~/.ssh/id_rsa

git clone --branch gh-pages git@github.com:$DOCS_REPO deploy_docs

cd deploy_docs
git config user.name "doc upload bot"
git config user.email "nobody@example.com"
rm -rf $PROJECT_NAME
mv ../target/doc $PROJECT_NAME
git add -A $PROJECT_NAME
git commit -qm "doc upload for $PROJECT_NAME ($TRAVIS_REPO_SLUG)"
git push -q origin gh-pages
