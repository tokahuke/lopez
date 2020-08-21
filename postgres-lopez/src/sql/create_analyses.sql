insert into
    analyses (wave_id, analysis_name)
select
    $1::integer,
    analysis_name
from
    unnest($2::text[]) as _ (analysis_name)
on conflict (wave_id, analysis_name) do nothing
returning
    analysis_name,
    analysis_id;
