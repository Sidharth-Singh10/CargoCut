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

# Quotient Filter: A Brief Overview

A **Quotient Filter** is a **probabilistic data structure** that is used for efficient membership testing, similar to **Bloom Filters**, but with some key differences. It is space-efficient, meaning it uses less memory for storing elements, and is particularly effective for applications requiring fast lookups with a small memory footprint.

## **Key Characteristics**
- **Space-efficient**: Stores elements in a compact form, reducing the amount of memory required.
- **Probabilistic**: Offers a tradeoff between memory usage and accuracy, meaning it can return false positives, but never false negatives.
- **No False Negatives**: A lookup will never incorrectly report that an element is not in the filter when it actually is.
- **False Positives**: It can return false positives, meaning it might incorrectly report that an element is in the filter when it is not.
- **Efficient for large-scale datasets**: Ideal for scenarios like URL lookup, duplicate detection, etc.
- **Scalable**: Can be distributed across multiple nodes for large systems, allowing for greater scalability and resilience.

## **How It Works**
1. **Hashing the Input**: Each element (e.g., a URL) is hashed, and the hash is split into two parts:
   - **Quotient**: The leading portion of the hash.
   - **Remainder**: The trailing portion of the hash.
2. **Storage**: The quotient is used as an index to place the remainder in a table. The remainder is stored in a bucket at the corresponding index.
3. **Lookups**: During lookups, the quotient part of the input is used to index into the filter. If the remainder is found in the corresponding bucket, the element is assumed to be in the filter (with a possibility of a false positive).
4. **No False Negatives**: If an element is present in the filter, it is guaranteed to be in the set. If the filter reports that an element is absent, it is definitely not in the set.

## **Benefits**
- **Space Efficiency**: The Quotient Filter is more space-efficient than traditional hash tables and even Bloom Filters, making it suitable for high-performance applications with memory constraints.
- **No False Negatives**: Unlike Bloom Filters, which may return false negatives, the Quotient Filter guarantees no false negatives, ensuring reliable lookups.
- **Scalability**: It can be distributed across nodes in a distributed system, allowing for horizontal scaling while maintaining its space efficiency.
- **Periodic Snapshots**: For persistence and fault tolerance, periodic snapshots of the filter can be saved to disk, ensuring data durability.
- **Automatic Recovery**: In the case of a failure, the Quotient Filter can be recovered from its snapshots, ensuring minimal downtime.

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




