.PHONY: build push deploy

IMAGE ?= ghcr.io/nantian-gw/echo-bench:latest

build:
	docker build -t $(IMAGE) .

push:
	docker push $(IMAGE)

deploy:
	kubectl apply -f deploy.yaml

# One-shot: build and push to GHCR
release: build
	docker push $(IMAGE)