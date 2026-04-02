window.BENCHMARK_DATA = {
  "lastUpdate": 1775097018433,
  "repoUrl": "https://github.com/AJenbo/phpantom_lsp",
  "entries": {
    "PHPantom Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "4fc27571246a61cd77f601060f20636204962d74",
          "message": "Add memory benchmark.",
          "timestamp": "2026-04-02T01:45:15+02:00",
          "tree_id": "04984455a7c5ceef33d2a60b1f62b9ba07c8436a",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4fc27571246a61cd77f601060f20636204962d74"
        },
        "date": 1775087790706,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.571,
            "range": "± 0.037",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.071,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.14,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.261,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.156,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.762,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.516,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.045,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.015,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.011,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.02,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.594,
            "range": "± 0.052",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.373,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.196,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.019,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "variable_completion/short",
            "value": 0.012,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.084,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.019,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.012,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.173,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.928,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.332,
            "range": "± 0.045",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.943,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0.000",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 81.203,
            "range": "± 0.701",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.485,
            "range": "± 0.010",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "b6ee3470cb01e83be243609b855fafb30f329b44",
          "message": "Add iterable return type code action for PHPStan",
          "timestamp": "2026-04-02T03:15:20+02:00",
          "tree_id": "0d756444188b23d3f88683c164f9546f3b57c024",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b6ee3470cb01e83be243609b855fafb30f329b44"
        },
        "date": 1775093057420,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.587,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.006,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.071,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.143,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.268,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.789,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.56,
            "range": "± 0.045",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.048,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.015,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.011,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.325,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.401,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.193,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.019,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/short",
            "value": 0.013,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.089,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.019,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.013,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.186,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.956,
            "range": "± 0.049",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.411,
            "range": "± 0.163",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.975,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 86.478,
            "range": "± 0.911",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.499,
            "range": "± 0.012",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "69c913ea9af25144f18947e4a7747efa63a777cb",
          "message": "Fix some keyword suggestions",
          "timestamp": "2026-04-02T04:21:22+02:00",
          "tree_id": "d0cfe5d4f51a98be3022117507601d64759eeeeb",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/69c913ea9af25144f18947e4a7747efa63a777cb"
        },
        "date": 1775097017652,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.578,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.006,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.071,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.273,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.166,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.789,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.568,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.015,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.019,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.257,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.407,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.019,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/short",
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.089,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.185,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.953,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.397,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.966,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.028,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 86.951,
            "range": "± 1.394",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.502,
            "range": "± 0.004",
            "unit": "ms"
          }
        ]
      }
    ]
  }
}