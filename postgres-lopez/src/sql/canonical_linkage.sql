select 
    *
from
    canonical_linkage
where
    wave_id = $1::integer;
