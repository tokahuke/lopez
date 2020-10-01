begin;

alter table analyses add column result_type text;
update analyses set result_type = 'any';
alter table analyses alter column result_type set not null;

end;
