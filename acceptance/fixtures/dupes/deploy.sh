#!/bin/bash
# Deployment script — builds docker image and pushes to registry
set -e
docker build -t myapp:latest .
docker push registry.example.com/myapp:latest
echo "Deployed"
