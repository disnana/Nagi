"""Failure-only closed facts. No raw child output, exceptions or paths."""
PHASES = {'binding','scope','capacity','ready','tls','negative','provision','browser','report',
          'stop_ack','close','ownership','cancelled','timeout','unexpected'}
STAGES = {'none','settings','native_wrong_ca','guard','native_positive','native_wrong_host',
          'engine_launch','engine_positive','engine_wrong_ca','engine_wrong_host','engine_close',
          'business_config','business','receipt','input'}
ENGINES = {'none','chromium','firefox','webkit'}


def validate(value, *, outer=False):
    keys = {'schema_version','stage','engine'} | ({'phase'} if outer else set())
    if (type(value) is not dict or set(value) != keys or type(value['schema_version']) is not int
            or value['schema_version'] != 1 or type(value['stage']) is not str or value['stage'] not in STAGES
            or type(value['engine']) is not str or value['engine'] not in ENGINES
            or (value['stage'].startswith('engine_') != (value['engine'] != 'none'))
            or (outer and (type(value['phase']) is not str or value['phase'] not in PHASES))):
        raise ValueError('invalid failure checkpoint')
    return {key:value[key] for key in keys}
