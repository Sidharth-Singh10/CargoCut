CREATE TABLE urls (
    short_code VARCHAR(50) NOT NULL,
    long_url TEXT NOT NULL,
    expiry_date DATE NOT NULL,
    PRIMARY KEY (short_code, expiry_date)
) PARTITION BY RANGE (expiry_date);

-- Create partition for February 2025
CREATE TABLE urls_y2025m02 PARTITION OF urls 
FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');

-- Create partition for March 2025
CREATE TABLE urls_y2025m03 PARTITION OF urls 
FOR VALUES FROM ('2025-03-01') TO ('2025-04-01');
