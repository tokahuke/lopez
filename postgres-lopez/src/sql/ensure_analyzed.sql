insert into
    analysis_results (wave_id, page_id, analysis_id, result)
select
    $1::integer,
    $2::bigint,
    analysis_id,
    result
from
    unnest($3::text[], $4::jsonb[]) as incoming (analysis_name, result)
        join analyses on incoming.analysis_name = analyses.analysis_name
            and analyses.wave_id = $1::integer;
