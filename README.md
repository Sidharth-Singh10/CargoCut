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

### Partitioning Strategy:

The shortened_urls table is partitioned by month-year basis to facilitate efficient cleanup of expired URLs. This allows us to:

- Drop entire partitions of expired URLs when all URLs in that partition are expired
- Run maintenance tasks only on specific partitions rather than the entire table
- Partition pruning improves recent URL query performance by allowing PostgreSQL to scan only the current month's partition (which likely contains most actively accessed URLs), keeping this "hot" data in memory, maintaining smaller indexes for faster lookups, and reducing lock contention between read/write operations.

Partitioning by month is superior to traditional row-by-row deletion because it allows for bulk partition drops, which are nearly instantaneous operations regardless of partition size, avoiding the heavy I/O, transaction logging, and index maintenance overhead of individual DELETE operations. Additionally, this approach prevents table fragmentation and reduces the need for frequent VACUUM operations, maintaining consistent query performance as the database grows. At the same time, it's highly useful in replication environments like we are using with CloudNative PG, with near <100ms average replication lag even for heavy datasets.


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

### Flow

### Write-Request-Scenario-1:
2 concurrent requests are sent out, one directly to the database and other to the quotient filter.

![image](https://github.com/user-attachments/assets/9b82c176-2211-4e6e-8329-94d9216fa813)

### Write-Request-Scenario-2:
If a write request to the database fails,and the request to the quotient filter succeeds , a rollback request is sent to the quotient filter which removes the entry from the quotient filter, since quotient filters support removal of elements.

![image](https://github.com/user-attachments/assets/39ea8eb6-ebb6-49af-bd3c-da1cbe885946)

### Read-Request-Scenario-1:
If in the read request short-url is not found in the quotient filter, Error 404 is returned to the user.

![image](https://github.com/user-attachments/assets/7b647db8-9094-4aea-9f77-152e2c88766c)

### Read-Request-Scenario-2:
If the short-url if found in the quotient filter then, redis is checked , if exists in redis, then short-url is successfully returned to user.

![image](https://github.com/user-attachments/assets/dc3cde35-217a-4254-8b12-e2a61e0c7f94)

### Read-Request-Scenario-3:
If the short-url if found in the quotient filter then, redis is checked , if it does not exists in redis, then a read-only replica is checked, if found,short-url is returned to the backend, at the same time a parallel request which is an independent process, whose task is to insert short_url(key) and original_url(value) into redis.

![image](https://github.com/user-attachments/assets/788bec82-48d7-4cd7-9fc8-5fb860469d20)

### Read-Request-Scenario-4:
![image](https://github.com/user-attachments/assets/3712d18f-b0a3-4ec8-a7ed-a2e5ec57a282)


### Service Startup and Recovery
![q6](https://github.com/user-attachments/assets/5d92a4ea-84c1-4620-b124-4b6a5a8e9dfe)


## 📷 Architecture Diagrams 
![q1](https://github.com/user-attachments/assets/fe0ea380-1d19-4fa6-ab57-0dea0418faae)
![q2](https://github.com/user-attachments/assets/3e862184-ce4c-4489-8c7c-fd8f85348017)




