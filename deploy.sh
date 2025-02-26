#!/bin/bash
set -e

# Replace with your values
AWS_ACCOUNT_ID="your-aws-account-id"
AWS_REGION="us-west-2"
ECR_REPOSITORY="url-shortener"
DB_PASSWORD=$(openssl rand -base64 32)

# Login to ECR
aws ecr get-login-password --region $AWS_REGION | docker login --username AWS --password-stdin $AWS_ACCOUNT_ID.dkr.ecr.$AWS_REGION.amazonaws.com

# Create ECR repository if it doesn't exist
aws ecr describe-repositories --repository-names $ECR_REPOSITORY --region $AWS_REGION || \
  aws ecr create-repository --repository-name $ECR_REPOSITORY --region $AWS_REGION

# Build and push Docker image
docker build -t $ECR_REPOSITORY:latest .
docker tag $ECR_REPOSITORY:latest $AWS_ACCOUNT_ID.dkr.ecr.$AWS_REGION.amazonaws.com/$ECR_REPOSITORY:latest
docker push $AWS_ACCOUNT_ID.dkr.ecr.$AWS_REGION.amazonaws.com/$ECR_REPOSITORY:latest

# Replace placeholders in Kubernetes YAML files
sed -i "s/\${AWS_ACCOUNT_ID}/$AWS_ACCOUNT_ID/g" k8s-deployment.yaml
sed -i "s/\${AWS_REGION}/$AWS_REGION/g" k8s-deployment.yaml
sed -i "s/\${DB_PASSWORD}/$DB_PASSWORD/g" k8s-deployment.yaml

echo "Generated database password: $DB_PASSWORD"
echo "Make sure to save this password somewhere safe!"

# Apply Kubernetes manifests
kubectl apply -f k8s-deployment.yaml
kubectl apply -f redis-deployment.yaml
kubectl apply -f ingress.yaml

# Wait for deployments to be ready
kubectl rollout status deployment/url-shortener
kubectl rollout status deployment/redis

echo "Deployment completed successfully!"