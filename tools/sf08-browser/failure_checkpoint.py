"""Failure-only closed facts. No raw child output, exceptions or paths."""
PHASES = {'binding','scope','capacity','ready','tls','negative','provision','browser','report',
          'stop_ack','close','ownership','cancelled','timeout','unexpected'}
STAGES = {'none','settings','native_wrong_ca','guard','native_positive','native_wrong_host',
          'engine_launch','engine_positive','engine_positive_ca','engine_positive_host','engine_positive_timeout',
          'engine_positive_response','engine_positive_tls_unknown','engine_positive_unknown',
          'engine_wrong_ca','engine_wrong_ca_response_received','engine_wrong_ca_bad_signature',
          'engine_wrong_ca_hostname','engine_wrong_ca_timeout','engine_wrong_ca_no_response',
          'engine_wrong_ca_other_tls','engine_wrong_ca_unknown',
          'engine_wrong_ca_webkit_protocol_form','engine_wrong_ca_webkit_token_present',
          'engine_wrong_ca_webkit_no_fixed_token',
          'engine_wrong_ca_webkit_cancelled_first_line','engine_wrong_ca_webkit_cancelled_call_log',
          'engine_wrong_ca_webkit_cancelled_other','engine_wrong_ca_webkit_shape_non_error',
          'engine_wrong_ca_webkit_shape_non_string','engine_wrong_ca_webkit_shape_oversize',
          'engine_wrong_ca_webkit_shape_access_failed',
          'engine_wrong_ca_diag_bad_signature_present','engine_wrong_ca_diag_unknown_issuer_present',
          'engine_wrong_ca_diag_hostname_present','engine_wrong_ca_diag_other_tls_present',
          'engine_wrong_ca_diag_multiple_codes','engine_wrong_ca_diag_no_fixed_code',
          'engine_wrong_ca_shape_non_error','engine_wrong_ca_shape_non_string',
          'engine_wrong_ca_shape_oversize','engine_wrong_ca_shape_access_failed','engine_wrong_host',
          'engine_wrong_host_page_open','engine_wrong_host_page_close','engine_wrong_host_response_received',
          'engine_wrong_host_no_response','engine_wrong_host_timeout','engine_wrong_host_unknown',
          'engine_wrong_host_webkit_protocol_form','engine_wrong_host_webkit_token_present',
          'engine_wrong_host_webkit_no_fixed_token','engine_wrong_host_webkit_cancelled_present',
          'engine_wrong_host_webkit_shape_non_error','engine_wrong_host_webkit_shape_non_string',
          'engine_wrong_host_webkit_shape_oversize','engine_wrong_host_webkit_shape_access_failed','engine_close',
          'business_config','business','receipt','input',
          'engine_wrong_host_one_shot_credential','engine_wrong_host_one_shot_setup',
          'engine_wrong_host_one_shot_navigation','engine_wrong_host_one_shot_request',
          'engine_wrong_host_one_shot_response','engine_wrong_host_one_shot_peer_legacy_certificate',
          'engine_wrong_host_one_shot_peer_other','engine_wrong_host_one_shot_peer_binding',
          'engine_wrong_host_one_shot_socket','engine_wrong_host_one_shot_lifecycle',
          'engine_wrong_host_one_shot_deadline','engine_wrong_host_one_shot_join',
          'engine_wrong_host_one_shot_cleanup','engine_wrong_host_one_shot_unknown'}
WEBKIT_WITNESS_DIAGNOSTICS = frozenset({
    'missing_event','duplicate_event','foreign_request_url','foreign_request_non_navigation',
    'foreign_request_frame','not_certificate',
    'not_certificate_empty','not_certificate_oversize','not_certificate_cancelled',
    'not_certificate_provisional_cancelled','not_certificate_certificate_form',
    'not_certificate_certificate_token','not_certificate_dns_token',
    'not_certificate_connection_token','not_certificate_cancel_token',
    'not_certificate_tls_token','not_certificate_multiple_tokens',
    'malformed','http_response','lifecycle','error_shape',
    'deadline','response_query_fault','response_query_timeout','certificate_event'})
WEBKIT_WITNESS_BASE_STAGES = frozenset({
    'engine_wrong_ca_unknown','engine_wrong_ca_response_received','engine_wrong_ca_no_response',
    'engine_wrong_ca_timeout','engine_wrong_ca_webkit_protocol_form','engine_wrong_ca_webkit_token_present',
    'engine_wrong_ca_webkit_cancelled_first_line','engine_wrong_ca_webkit_cancelled_call_log',
    'engine_wrong_ca_webkit_cancelled_other','engine_wrong_ca_webkit_no_fixed_token',
    'engine_wrong_ca_webkit_shape_non_error','engine_wrong_ca_webkit_shape_non_string',
    'engine_wrong_ca_webkit_shape_oversize','engine_wrong_ca_webkit_shape_access_failed',
    'engine_wrong_host_unknown','engine_wrong_host_response_received','engine_wrong_host_no_response',
    'engine_wrong_host_timeout','engine_wrong_host_webkit_protocol_form','engine_wrong_host_webkit_token_present',
    'engine_wrong_host_webkit_cancelled_present','engine_wrong_host_webkit_no_fixed_token',
    'engine_wrong_host_webkit_shape_non_error','engine_wrong_host_webkit_shape_non_string',
    'engine_wrong_host_webkit_shape_oversize','engine_wrong_host_webkit_shape_access_failed'})
WEBKIT_WITNESS_STAGES = frozenset({
    f'{base}_witness_{diagnostic}'
    for base in WEBKIT_WITNESS_BASE_STAGES for diagnostic in WEBKIT_WITNESS_DIAGNOSTICS})
FIREFOX_WRONG_CA_DIAGNOSTIC_STAGES = {
    'engine_wrong_ca_diag_bad_signature_present','engine_wrong_ca_diag_unknown_issuer_present',
    'engine_wrong_ca_diag_hostname_present','engine_wrong_ca_diag_other_tls_present',
    'engine_wrong_ca_diag_multiple_codes','engine_wrong_ca_diag_no_fixed_code',
    'engine_wrong_ca_shape_non_error','engine_wrong_ca_shape_non_string',
    'engine_wrong_ca_shape_oversize','engine_wrong_ca_shape_access_failed'}
WEBKIT_WRONG_CA_DIAGNOSTIC_STAGES = {
    'engine_wrong_ca_webkit_shape_non_string','engine_wrong_ca_webkit_shape_oversize',
    'engine_wrong_ca_webkit_shape_access_failed','engine_wrong_ca_webkit_protocol_form',
    'engine_wrong_ca_webkit_shape_non_error','engine_wrong_ca_webkit_token_present',
    'engine_wrong_ca_webkit_no_fixed_token','engine_wrong_ca_webkit_cancelled_first_line',
    'engine_wrong_ca_webkit_cancelled_call_log','engine_wrong_ca_webkit_cancelled_other'}
WEBKIT_WRONG_HOST_DIAGNOSTIC_STAGES = {
    'engine_wrong_host_webkit_protocol_form','engine_wrong_host_webkit_token_present',
    'engine_wrong_host_webkit_no_fixed_token','engine_wrong_host_webkit_cancelled_present',
    'engine_wrong_host_webkit_shape_non_error','engine_wrong_host_webkit_shape_non_string',
    'engine_wrong_host_webkit_shape_oversize','engine_wrong_host_webkit_shape_access_failed'}
ENGINES = {'none','chromium','firefox','webkit'}


def validate(value, *, outer=False):
    keys = {'schema_version','stage','engine'} | ({'phase'} if outer else set())
    if (type(value) is not dict or set(value) != keys or type(value['schema_version']) is not int
            or value['schema_version'] != 1 or type(value['stage']) is not str
            or (value['stage'] not in STAGES and value['stage'] not in WEBKIT_WITNESS_STAGES)
            or type(value['engine']) is not str or value['engine'] not in ENGINES
            or (value['stage'].startswith('engine_') != (value['engine'] != 'none'))
            or (value['stage'] in FIREFOX_WRONG_CA_DIAGNOSTIC_STAGES and value['engine'] != 'firefox')
            or (value['stage'] in WEBKIT_WRONG_CA_DIAGNOSTIC_STAGES and value['engine'] != 'webkit')
            or (value['stage'] in WEBKIT_WRONG_HOST_DIAGNOSTIC_STAGES and value['engine'] != 'webkit')
            or (value['stage'].startswith('engine_wrong_host_one_shot_') and value['engine'] != 'webkit')
            or (value['stage'] in WEBKIT_WITNESS_STAGES and value['engine'] != 'webkit')
            or (value['stage'] in {'engine_wrong_ca_bad_signature','engine_wrong_ca_other_tls'}
                and value['engine'] != 'firefox')
            or (value['stage'] == 'engine_wrong_ca_hostname' and value['engine'] == 'webkit')
            or (outer and (type(value['phase']) is not str or value['phase'] not in PHASES))):
        raise ValueError('invalid failure checkpoint')
    return {key:value[key] for key in keys}
