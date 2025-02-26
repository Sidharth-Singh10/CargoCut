#!/bin/bash
set -e

# Set variables
CLUSTER_NAME="url-shortener-cluster"
AWS_REGION="ap-south-1"
K8S_VERSION="1.28"

# Create EKS cluster
echo "Creating EKS cluster..."
eksctl create cluster \
  --name $CLUSTER_NAME \
  --region $AWS_REGION \
  --version $K8S_VERSION \
  --nodegroup-name standard-nodes \
  --node-type t3.micro \
  --nodes 2 \
  --nodes-min 1 \
  --nodes-max 3 \
  --with-oidc \
  --managed

# Install AWS Load Balancer Controller
echo "Installing AWS Load Balancer Controller..."
# Download IAM policy document
curl -o iam-policy.json https://raw.githubusercontent.com/kubernetes-sigs/aws-load-balancer-controller/main/docs/install/iam_policy.json

# Create IAM policy
aws iam create-policy \
  --policy-name AWSLoadBalancerControllerIAMPolicy \
  --policy-document file://iam-policy.json

# Create service account
eksctl create iamserviceaccount \
  --cluster=$CLUSTER_NAME \
  --namespace=kube-system \
  --name=aws-load-balancer-controller \
  --attach-policy-arn=arn:aws:iam::$(aws sts get-caller-identity --query Account --output text):policy/AWSLoadBalancerControllerIAMPolicy \
  --override-existing-serviceaccounts \
  --approve

# Install the AWS Load Balancer Controller using Helm
helm repo add eks https://aws.github.io/eks-charts
helm repo update
helm install aws-load-balancer-controller eks/aws-load-balancer-controller \
  -n kube-system \
  --set clusterName=$CLUSTER_NAME \
  --set serviceAccount.create=false \
  --set serviceAccount.name=aws-load-balancer-controller

# Install CloudNative PostgreSQL Operator
echo "Installing CloudNative PostgreSQL Operator..."
kubectl apply --server-side -f \
  https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/release-1.25/releases/cnpg-1.25.0.yaml

# Wait for the operator to be ready
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=cloudnative-pg -n cnpg-system --timeout=120s

# Create PostgreSQL cluster
echo "Creating PostgreSQL cluster..."
kubectl apply -f - <<EOF
---
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: cluster-with-metrics
spec:
  instances: 3
  storage:
    size: 1Gi
  monitoring:
    enablePodMonitor: true
EOF

echo "EKS cluster and required components have been set up successfully!"
echo "You can now deploy your URL shortener application using the deploy script."