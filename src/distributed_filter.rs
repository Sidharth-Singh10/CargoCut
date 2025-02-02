use chrono::{Datelike, NaiveDate, Utc};
use qfilter::Filter;
use std::collections::HashMap;
use tracing::info;

// Structure to hold filter information for a partition
#[derive(Debug)]
pub struct PartitionFilter {
    pub filter: Filter,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

// Main structure to manage distributed filters
#[derive(Debug)]
pub struct DistributedFilter {
    pub filters: HashMap<String, PartitionFilter>,
    pub future_partition: Option<PartitionFilter>,
}

impl DistributedFilter {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(DistributedFilter {
            filters: HashMap::new(),
            future_partition: None,
        })
    }

    // Initialize a new partition filter
    pub fn create_partition_filter(
        &mut self,
        partition_name: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let filter = Filter::new(1_000_000, 0.01)?; // Adjust size based on expected partition size
        let partition_filter = PartitionFilter {
            filter,
            start_date,
            end_date,
        };

        self.filters.insert(partition_name, partition_filter);
        Ok(())
    }

    // Check if a short code exists in any relevant filter
    pub fn contains(&self, short_code: &str, current_date: NaiveDate) -> bool {
        // Check current partitions
        // println!("curr")
        info!("Checking current partitions");
        
        for filter in self.filters.values() {
            if filter.filter.contains(short_code) {
                return true;
            }
        }

        // Check future partition if exists
        if let Some(future) = &self.future_partition {
            if current_date > future.start_date {
                return future.filter.contains(short_code);
            }
        }

        false
    }

    // Insert a short code into the appropriate filter
    pub fn insert(
        &mut self,
        short_code: &str,
        expiry_date: NaiveDate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let partition_name = generate_partition_name(expiry_date);
        println!(
            "inserting into fileter partition_name: {:?}",
            partition_name
        );
        if let Some(partition_filter) = self.filters.get_mut(&partition_name) {
            partition_filter.filter.insert(short_code)?;
        } else if let Some(future) = &mut self.future_partition {
            if expiry_date > future.start_date {
                future.filter.insert(short_code)?;
            }
        }

        Ok(())
    }

    // // Remove expired partitions and their corresponding filters
    // pub async fn cleanup_expired_partitions(
    //     &mut self,
    //     pool: &PgPool,
    // ) -> Result<(), Box<dyn std::error::Error>> {
    //     let current_date = Utc::now().naive_utc().date();

    //     // Remove expired filters
    //     self.filters
    //         .retain(|_, filter| filter.end_date >= current_date);

    //     // Create new partition from future if needed
    //     if let Some(future) = &self.future_partition {
    //         if future.start_date
    //             <= current_date
    //                 .checked_add_months(chrono::Months::new(36))
    //                 .unwrap()
    //         {
    //             // Move data from future to new partition
    //             let new_partition_name = generate_partition_name(future.start_date);
    //             let new_filter = Filter::new(1_000_000, 0.01)?;

    //             // Create new partition in database
    //             create_new_partition(
    //                 pool,
    //                 &new_partition_name,
    //                 future.start_date,
    //                 future.end_date,
    //             )
    //             .await?;

    //             self.filters.insert(
    //                 new_partition_name,
    //                 PartitionFilter {
    //                     filter: new_filter,
    //                     start_date: future.start_date,
    //                     end_date: future.end_date,
    //                 },
    //             );

    //             // Reset future partition
    //             self.future_partition = None;
    //         }
    //     }

    //     Ok(())
    // }
}

// Helper function to generate partition name
pub fn generate_partition_name(date: NaiveDate) -> String {
    format!("urls_y{}m{:02}", date.year(), date.month())
}

// Helper function to create a new partition
// pub async fn create_new_partition(
//     pool: &PgPool,
//     partition_name: &str,
//     start_date: NaiveDate,
//     end_date: NaiveDate,
// ) -> Result<(), Box<dyn std::error::Error>> {
//     let query = format!(
//         "CREATE TABLE IF NOT EXISTS {}
//          PARTITION OF urls
//          FOR VALUES FROM ('{}') TO ('{}');",
//         partition_name, start_date, end_date
//     );

//     sqlx::query(&query).execute(pool).await?;
//     Ok(())
// }

// Initialize the distributed filter system
// pub async fn initialize_distributed_filter_system(
//     pool: &PgPool,
// ) -> Result<Arc<Mutex<DistributedFilter>>, Box<dyn std::error::Error>> {
//     let mut distributed_filter = DistributedFilter::new()?;

//     // Initialize partitions for the next 36 months
//     let current_date = Utc::now().naive_utc().date();

//     for months_ahead in 0..36 {
//         let start_date = current_date
//             .checked_add_months(chrono::Months::new(months_ahead))
//             .unwrap()
//             .with_day(1)
//             .unwrap();

//         let end_date = start_date
//             .checked_add_months(chrono::Months::new(1))
//             .unwrap();

//         let partition_name = generate_partition_name(start_date);
//         distributed_filter.create_partition_filter(partition_name, start_date, end_date)?;
//     }

//     // Initialize future partition
//     let future_start = current_date
//         .checked_add_months(chrono::Months::new(36))
//         .unwrap();
//     let future_end = NaiveDate::MAX;

//     distributed_filter.future_partition = Some(PartitionFilter {
//         filter: Filter::new(1_000_000, 0.01)?,
//         start_date: future_start,
//         end_date: future_end,
//     });

//     Ok(Arc::new(Mutex::new(distributed_filter)))
// }
