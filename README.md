# CargoCut - A blazingly Fast Performant URL Shortener

A cloud-native, distributed URL shortening service built with Rust, featuring high throughput, low latency, and robust data consistency.

## 🚀 Features

- Lightning-fast URL shortening with distributed architecture
- Automatic data partitioning based on expiry dates
- Multi-layer caching with Redis and Quotient Filter
- Asynchronous backup to Amazon S3 
- High availability with PostgreSQL replication
- Efficient space utilization with probabilistic data structures
- Automatic service recovery and snapshot management

## 🛠️ Tech Stack

- **Backend**: Rust
- **Database**: Distributed Cloud-Native PostgreSQL
- **Caching**: Redis
- **Storage**: Amazon S3 (for Quotient Filter backups)
- **Probabilistic Data Structure**: Quotient Filter (qfilter crate)

## 🏗️ Architecture

### Database Structure

The service uses a partitioned database schema:
- Main table with columns: `short_code`, `long_code`, `expiry_date`
- Automatic partitioning based on 36-month intervals
- Future table for URLs with expiry > 36 months

### Distributed Quotient Filter

The service implements a Quotient Filter (similar to Bloom filters) for efficient URL lookup:
- Space-efficient probabilistic data structure
- Helps prevent duplicate short URLs
- Distributed across nodes for scalability
- Periodic snapshots for persistence
- Automatic recovery from failures

Key advantages over traditional Bloom filters:
- Support for deletions
- Better locality of reference  
- Lower false positive rate at similar space usage
- Efficient resizing capabilities

## 📊 System Design

### URL Writing Process
![q4](https://github.com/user-attachments/assets/77512f6e-9528-4c08-add2-ae20aa3a6621)


### URL Reading Process 
![q5](https://github.com/user-attachments/assets/db40a7b1-7c5b-43da-88e3-f3c64c5c0d5d)


### Service Startup and Recovery
![q6](https://github.com/user-attachments/assets/5d92a4ea-84c1-4620-b124-4b6a5a8e9dfe)


## 📷 Architecture Diagrams 
![q3](https://github.com/user-attachments/assets/cd003806-59ce-41a9-8cbb-6dab4af07218)
![q1](https://github.com/user-attachments/assets/fe0ea380-1d19-4fa6-ab57-0dea0418faae)
![q2](https://github.com/user-attachments/assets/3e862184-ce4c-4489-8c7c-fd8f85348017)




