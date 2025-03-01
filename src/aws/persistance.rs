use aws_sdk_s3::Client;
use bincode;
use bytes::Bytes;
use chrono::{Datelike, NaiveDate, Utc};
use qfilter::Filter;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::distributed_filter::{generate_partition_name, DistributedFilter, PartitionFilter};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DistributedFilterSnapshot {
    partition_filters: HashMap<String, PartitionFilterData>,
    future_partition: Option<PartitionFilterData>,
    metadata: SnapshotMetadata,
}

// Separate the filter data from the actual Filter instance
#[derive(Serialize, Deserialize, Debug, Clone)]
struct PartitionFilterData {
    filter_data: Vec<u8>, // Raw bytes of the filter
    start_date: NaiveDate,
    end_date: NaiveDate,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SnapshotMetadata {
    timestamp: chrono::DateTime<Utc>,
    total_items_count: u64,
    partition_count: usize,
}

pub struct DistributedFilterPersistence {
    s3_client: Client,
    bucket: String,
    prefix: String,
}

impl DistributedFilterPersistence {
    pub async fn new(bucket: String, prefix: String) -> Result<Self, Box<dyn Error>> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let s3_client = Client::new(&config);

        Ok(DistributedFilterPersistence {
            s3_client,
            bucket,
            prefix,
        })
    }

    pub async fn save_snapshot(
        &self,
        distributed_filter: &DistributedFilter,
    ) -> Result<String, Box<dyn Error>> {
        let mut partition_filters = HashMap::new();
        let mut total_items = 0u64;

        // Serialize each filter separately
        for (name, filter) in &distributed_filter.filters {
            let filter_bytes = bincode::serialize(&filter.filter)?;
            partition_filters.insert(
                name.clone(),
                PartitionFilterData {
                    filter_data: filter_bytes,
                    start_date: filter.start_date,
                    end_date: filter.end_date,
                },
            );
            total_items += filter.filter.len();
        }

        // Handle future partition
        let future_partition = distributed_filter.future_partition.as_ref().map(|f| {
            let filter_bytes = bincode::serialize(&f.filter).unwrap();
            PartitionFilterData {
                filter_data: filter_bytes,
                start_date: f.start_date,
                end_date: f.end_date,
            }
        });

        let snapshot = DistributedFilterSnapshot {
            partition_filters,
            future_partition,
            metadata: SnapshotMetadata {
                timestamp: Utc::now(),
                total_items_count: total_items,
                partition_count: distributed_filter.filters.len(),
            },
        };

        // Serialize the entire snapshot
        let serialized = bincode::serialize(&snapshot)?;
        tracing::info!("Serialized data size: {} bytes", serialized.len());

        let key = format!(
            "{}/distributed_filter_{}.bin",
            self.prefix,
            Utc::now().format("%Y%m%d_%H%M%S")
        );

        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(Bytes::from(serialized).into())
            .send()
            .await?;

        Ok(key)
    }

    pub async fn load_latest_snapshot(&self) -> Result<Option<DistributedFilter>, Box<dyn Error>> {
        let objects = self
            .s3_client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&self.prefix)
            .send()
            .await?;

        let slice_of_objects = objects.contents();

        let latest = match slice_of_objects
            .iter()
            .filter_map(|obj| obj.last_modified.map(|lm| (lm, obj)))
            .max_by_key(|&(lm, _)| lm)
            .map(|(_, obj)| obj)
        {
            Some(obj) => obj,
            None => return Ok(None),
        };

        let response = self
            .s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(latest.key().unwrap())
            .send()
            .await?;

        let data = response.body.collect().await?.into_bytes();

        // Deserialize the snapshot
        let snapshot: DistributedFilterSnapshot = bincode::deserialize(&data)?;

        // Reconstruct DistributedFilter
        let mut distributed_filter = DistributedFilter::new()?;

        // Restore partition filters
        for (name, filter_data) in snapshot.partition_filters {
            // Deserialize the filter from bytes
            let filter: Filter = bincode::deserialize(&filter_data.filter_data)?;

            distributed_filter.filters.insert(
                name,
                PartitionFilter {
                    filter,
                    start_date: filter_data.start_date,
                    end_date: filter_data.end_date,
                },
            );
        }

        // Restore future partition if it exists
        if let Some(future_data) = snapshot.future_partition {
            let future_filter: Filter = bincode::deserialize(&future_data.filter_data)?;
            distributed_filter.future_partition = Some(PartitionFilter {
                filter: future_filter,
                start_date: future_data.start_date,
                end_date: future_data.end_date,
            });
        }

        tracing::info!(
            "Restored distributed filter with {} partitions and {} total items",
            snapshot.metadata.partition_count,
            snapshot.metadata.total_items_count
        );

        Ok(Some(distributed_filter))
    }

    // pub async fn cleanup_old_snapshots(&self, keep_last_n: i32) -> Result<(), Box<dyn Error>> {
    //     let objects = self
    //         .s3_client
    //         .list_objects_v2()
    //         .bucket(&self.bucket)
    //         .prefix(&self.prefix)
    //         .send()
    //         .await?;

    //     if let Some(contents) = objects.contents() {
    //         let mut snapshots: Vec<_> = contents
    //             .iter()
    //             .filter(|obj| obj.key().unwrap_or("").contains("distributed_filter_"))
    //             .collect();

    //         snapshots.sort_by_key(|obj| obj.last_modified().unwrap_or_default());

    //         if snapshots.len() > keep_last_n as usize {
    //             for obj in snapshots.iter().take(snapshots.len() - keep_last_n as usize) {
    //                 self.s3_client
    //                     .delete_object()
    //                     .bucket(&self.bucket)
    //                     .key(obj.key().unwrap())
    //                     .send()
    //                     .await?;
    //                 tracing::info!("Deleted old snapshot: {}", obj.key().unwrap());
    //             }
    //         }
    //     }

    //     Ok(())
    // }
}

// Update initialize_distributed_filter_system to use persistence
pub async fn initialize_distributed_filter_system(
    _pool: &PgPool,
    bucket: String,
    prefix: String,
) -> Result<Arc<Mutex<DistributedFilter>>, Box<dyn Error>> {
    let _persistence = DistributedFilterPersistence::new(bucket, prefix).await?;

    // Try to load the latest snapshot
    // if let Some(distributed_filter) = persistence.load_latest_snapshot().await? {
    //     tracing::info!("Successfully restored distributed filter from snapshot");
    //     return Ok(Arc::new(Mutex::new(distributed_filter)));
    // }

    // If no snapshot exists, create a new distributed filter
    tracing::info!("No snapshot found, creating new distributed filter");
    let mut distributed_filter = DistributedFilter::new()?;

    // Initialize partitions for the next 36 months
    let current_date = Utc::now().naive_utc().date();

    for months_ahead in 0..36 {
        let start_date = current_date
            .checked_add_months(chrono::Months::new(months_ahead))
            .unwrap()
            .with_day(1)
            .unwrap();

        let end_date = start_date
            .checked_add_months(chrono::Months::new(1))
            .unwrap();

        let partition_name = generate_partition_name(start_date);
        distributed_filter.create_partition_filter(partition_name, start_date, end_date)?;
    }

    // Initialize future partition
    let future_start = current_date
        .checked_add_months(chrono::Months::new(36))
        .unwrap();
    let future_end = NaiveDate::MAX;

    distributed_filter.future_partition = Some(PartitionFilter {
        filter: Filter::new(1_000_000, 0.01)?,
        start_date: future_start,
        end_date: future_end,
    });

    Ok(Arc::new(Mutex::new(distributed_filter)))
}

// Add snapshot service for distributed filter
pub async fn run_distributed_snapshot_service(
    distributed_filter: Arc<Mutex<DistributedFilter>>,
    persistence: Arc<DistributedFilterPersistence>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(12000)); // Save every hour

    loop {
        interval.tick().await;
        match persistence
            .save_snapshot(&*distributed_filter.lock().await)
            .await
        {
            Ok(key) => tracing::info!("Successfully saved distributed filter snapshot: {}", key),
            Err(e) => tracing::error!("Failed to save distributed filter snapshot: {}", e),
        }

        // // Cleanup old snapshots, keeping last 24
        // if let Err(e) = persistence.cleanup_old_snapshots(24).await {
        //     tracing::error!("Failed to cleanup old snapshots: {}", e);
        // }
    }
}
