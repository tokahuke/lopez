insert into
    analyses (wave_id, analysis_name, result_type)
select
    $1::integer,
    analysis_name,
    result_type
from
    unnest($2::text[], $3::text[]) as _ (analysis_name, result_type)
on conflict (wave_id, analysis_name) do nothing
returning
    analysis_name,
    analysis_id;
