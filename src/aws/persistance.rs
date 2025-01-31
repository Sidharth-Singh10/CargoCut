use aws_sdk_s3::Client;
use bincode;
use bincode::Options;
use bytes::Bytes;
use chrono::Utc;
use qfilter::Filter;
use serde::{Deserialize, Serialize};
use std::error::Error;
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FilterSnapshot {
    filter: Filter,
    metadata: SnapshotMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SnapshotMetadata {
    items_count: u64,
    capacity: u64,
    error_ratio: f64,
}

pub struct FilterPersistence {
    s3_client: Client,
    bucket: String,
    prefix: String,
}

impl FilterPersistence {
    pub async fn new(bucket: String, prefix: String) -> Result<Self, Box<dyn Error>> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let s3_client = Client::new(&config);

        Ok(FilterPersistence {
            s3_client,
            bucket,
            prefix,
        })
    }

    pub async fn save_snapshot(&self, filter: &Filter) -> Result<String, Box<dyn Error>> {
        let snapshot = FilterSnapshot {
            filter: filter.clone(),
            metadata: SnapshotMetadata {
                items_count: filter.len(),
                capacity: filter.capacity(),
                error_ratio: filter.max_error_ratio(),
            },
        };

        // Serialize using bincode
        let serialized = bincode::DefaultOptions::new().serialize(&snapshot)?;
        tracing::info!("Serialized data size: {} bytes", serialized.len());

        // Create a unique key for this snapshot
        let key = format!(
            "{}/filter_{}.bin",
            self.prefix,
            Utc::now().format("%Y%m%d_%H%M%S")
        );

        // Upload to S3
        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(Bytes::from(serialized).into())
            .send()
            .await?;

        Ok(key)
    }

    async fn load_latest_snapshot(&self) -> Result<Option<Filter>, Box<dyn Error>> {
        // Scope for Improvement , better fetching of objects
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

        // Download the latest snapshot
        let response = self
            .s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(latest.key().unwrap())
            .send()
            .await?;

        tracing::info!("Downloading snapshot: {:#?}", response);

        // Read the body
        let data = response.body.collect().await?.into_bytes();

        tracing::info!("Downloaded data size: {} bytes", data.len());

        // Deserialize
        let snapshot: FilterSnapshot = bincode::DefaultOptions::new()
            .allow_trailing_bytes() // Important - allows trailing bytes in case they exist
            .deserialize(&data)
            .map_err(|e| {
                tracing::error!("Deserialization error: {:?}", e);
                e
            })?;
        tracing::info!("snapshot snapsho");
        Ok(Some(snapshot.filter))
    }
}

pub async fn initialize_filter_service() -> Result<Filter, Box<dyn Error>> {
    let persistence =
        FilterPersistence::new("affinitys3".to_string(), "qfilter-backups".to_string()).await?;

    // Try to load the latest snapshot
    match persistence.load_latest_snapshot().await? {
        Some(filter) => {
            // Verify filter integrity
            if filter.len() > 0 {
                tracing::info!("Restored filter with {} items", filter.len());
                Ok(filter)
            } else {
                tracing::error!(
                    "Backup filter is empty. Creating new filter with default settings"
                );
                Ok(Filter::new(10_000_000, 0.01)?)
            }
        }
        None => {
            tracing::error!("No backup found, creating new filter");
            Ok(Filter::new(10_000_000, 0.01)?)
        }
    }
}

pub async fn run_snapshot_service(
    filter: std::sync::Arc<tokio::sync::Mutex<Filter>>,
    persistence: std::sync::Arc<FilterPersistence>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(120)); // Save every hour, adjust as needed

    loop {
        interval.tick().await;
        match persistence.save_snapshot(&*filter.lock().await).await {
            Ok(_) => tracing::info!("Successfully saved filter snapshot"),
            Err(e) => tracing::error!("Failed to save filter snapshot: {}", e),
        }
    }
}

// Implement CleanUP

// pub async fn cleanup_old_snapshots(&self, keep_last_n: i32) -> Result<(), Box<dyn Error>> {
//     let objects = self
//         .s3_client
//         .list_objects_v2()
//         .bucket(&self.bucket)
//         .prefix(&self.prefix)
//         .send()
//         .await?;

//     if let Some(contents) = objects.contents() {
//         // Sort by last modified time
//         let mut snapshots: Vec<_> = contents
//             .iter()
//             .filter(|obj| obj.key().unwrap_or("").ends_with(".bin"))
//             .collect();

//         snapshots.sort_by_key(|obj| obj.last_modified().unwrap_or_default());

//         // Remove old snapshots, keeping the last n
//         if snapshots.len() > keep_last_n as usize {
//             for obj in snapshots
//                 .iter()
//                 .take(snapshots.len() - keep_last_n as usize)
//             {
//                 self.s3_client
//                     .delete_object()
//                     .bucket(&self.bucket)
//                     .key(obj.key().unwrap())
//                     .send()
//                     .await?;
//             }
//         }
//     }

//     Ok(())
// }

// Example usage
// async fn example_usage() -> Result<(), Box<dyn Error>> {
//     // Initialize persistence with your bucket and prefix
//     let persistence = FilterPersistence::new(
//         "your-bucket-name".to_string(),
//         "qfilter-snapshots".to_string(),
//     )
//     .await?;

//     // Create a new filter
//     let mut filter = Filter::new(1000000, 0.01)?;

//     // Add some items...
//     filter.insert("example1")?;
//     filter.insert("example2")?;

//     // Save snapshot
//     let snapshot_key = persistence.save_snapshot(&filter).await?;
//     println!("Saved snapshot: {}", snapshot_key);

//     // Later, load the latest snapshot
//     if let Some(restored_filter) = persistence.load_latest_snapshot().await? {
//         println!("Loaded filter with {} items", restored_filter.len());
//     }

//     Ok(())
// }
