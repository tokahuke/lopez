select
    count(*) as crawled
from
    "status" join wave_name using (wave_id)
where
    search_status = 'closed' and wave_name = $1::text
