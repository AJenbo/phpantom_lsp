window.BENCHMARK_DATA = {
  "lastUpdate": 1776820146255,
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
          "id": "e476f7fcdc9ac7638bba884d583f9acadde54eb3",
          "message": "Add link to memory benchmark",
          "timestamp": "2026-04-02T04:31:13+02:00",
          "tree_id": "e95b8d282c1f929321c821ab43aeeec8189c4699",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/e476f7fcdc9ac7638bba884d583f9acadde54eb3"
        },
        "date": 1775097594684,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.599,
            "range": "± 0.059",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.142,
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
            "value": 0.169,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.795,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.588,
            "range": "± 0.023",
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
            "value": 0.014,
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
            "value": 0.02,
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
            "value": 0.406,
            "range": "± 0.012",
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
            "value": 0.087,
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
            "value": 0.013,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.186,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.95,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.387,
            "range": "± 0.074",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.967,
            "range": "± 0.004",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.525,
            "range": "± 0.584",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.507,
            "range": "± 0.005",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "mail@matthias-gutjahr.de",
            "name": "Matthias Gutjahr",
            "username": "mattsches"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "732c34e506e568894bb9eb24a1e5cc60c1507ccb",
          "message": "Update SETUP.md\n\n--init is an option, not a command.",
          "timestamp": "2026-04-02T17:40:24+02:00",
          "tree_id": "ec1c0f4a904102b0f33435c4c5a141ae3d089d06",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/732c34e506e568894bb9eb24a1e5cc60c1507ccb"
        },
        "date": 1775144948207,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.595,
            "range": "± 0.011",
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
            "value": 0.072,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.142,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.169,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.802,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.587,
            "range": "± 0.024",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.319,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.41,
            "range": "± 0.013",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.087,
            "range": "± 0.003",
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
            "value": 0.186,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.959,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.433,
            "range": "± 0.043",
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
            "value": 87.413,
            "range": "± 1.061",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.501,
            "range": "± 0.005",
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
          "id": "289af94a33f563c7f90e6886b6d8d5c8d9611eb4",
          "message": "Add bug backlog for analyzer and Eloquent issues, docblock property\nwhere-methods",
          "timestamp": "2026-04-03T12:45:44+02:00",
          "tree_id": "bb4a43efb0e59dce043bd75bc4b5fb4baac75d6d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/289af94a33f563c7f90e6886b6d8d5c8d9611eb4"
        },
        "date": 1775213680551,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.593,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
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
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.794,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.578,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.048,
            "range": "± 0.001",
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
            "value": 3.362,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.388,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.195,
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
            "value": 0.087,
            "range": "± 0.002",
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
            "value": 0.187,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.962,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.407,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.976,
            "range": "± 0.004",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 85.365,
            "range": "± 0.24",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.525,
            "range": "± 0.007",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "metrofindings@gmail.com",
            "name": "Mark Kimsal",
            "username": "markkimsal"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "1ec76757e200a490e76db769b6ad51c2bc978535",
          "message": "ft: add dummy stdio cli flag for compatibility with LSP wrappers",
          "timestamp": "2026-04-03T20:08:09+02:00",
          "tree_id": "3e6ed0796d3a690aba3cd6453cc680ecb7bbe332",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/1ec76757e200a490e76db769b6ad51c2bc978535"
        },
        "date": 1775240213621,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.608,
            "range": "± 0.02",
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
            "value": 0.143,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.797,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.582,
            "range": "± 0.014",
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
            "value": 3.366,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.401,
            "range": "± 0.022",
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
            "value": 0.087,
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
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.982,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.524,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.982,
            "range": "± 0.014",
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
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 85.914,
            "range": "± 0.47",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.512,
            "range": "± 0.005",
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
          "id": "eafb028c4ed9095b6b8ee89f313039e07e8bf151",
          "message": "Track variable reassignment inside try/catch/finally blocks",
          "timestamp": "2026-04-04T00:18:27+02:00",
          "tree_id": "dbe72b1e2153852f80bedc97d8da29b6bdc51ac4",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/eafb028c4ed9095b6b8ee89f313039e07e8bf151"
        },
        "date": 1775255266843,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.58,
            "range": "± 0.033",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.276,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.788,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.56,
            "range": "± 0.011",
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.321,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.4,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.193,
            "range": "± 0.002",
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
            "value": 0.087,
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
            "value": 0.187,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.963,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.449,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.973,
            "range": "± 0.008",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 86.922,
            "range": "± 0.321",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.51,
            "range": "± 0.007",
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
          "id": "46172a94bbb102f9424b7fed1b0da3ad92e37e10",
          "message": "Fix relationship property and BelongsTo diagnostics for Laravel models",
          "timestamp": "2026-04-04T00:32:17+02:00",
          "tree_id": "82937b0c85adb7a2bb262d9e89cc1bdfeaaab6da",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/46172a94bbb102f9424b7fed1b0da3ad92e37e10"
        },
        "date": 1775256062391,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.629,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.07,
            "range": "± 0.001",
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
            "value": 0.269,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.773,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.525,
            "range": "± 0.01",
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
            "range": "± 0",
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
            "value": 3.583,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.392,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.003",
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
            "value": 0.086,
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
            "value": 0.188,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.964,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.46,
            "range": "± 0.064",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.972,
            "range": "± 0.028",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 85.907,
            "range": "± 0.238",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.502,
            "range": "± 0.005",
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
          "id": "004ef4feddcb8c11516bc6b64cf25e4f259b1435",
          "message": "Fix closure param inference for function-level templates and property\nchains",
          "timestamp": "2026-04-04T01:04:46+02:00",
          "tree_id": "266b5364f604051910a4adbadfb34e2c171e2ee9",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/004ef4feddcb8c11516bc6b64cf25e4f259b1435"
        },
        "date": 1775258036211,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.599,
            "range": "± 0.084",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
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
            "value": 0.142,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.772,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.535,
            "range": "± 0.036",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.048,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.019,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.396,
            "range": "± 0.105",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.39,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.083,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.018,
            "range": "± 0.001",
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
            "value": 0.179,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.916,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.158,
            "range": "± 0.084",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.933,
            "range": "± 0.044",
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
            "value": 0.031,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 82.905,
            "range": "± 1.866",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.494,
            "range": "± 0.013",
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
          "id": "22db360d53d73a76feb712ca1bda5ffbff81d29d",
          "message": "Add PHPStan `*` wildcard support in generic type positions",
          "timestamp": "2026-04-04T03:28:56+02:00",
          "tree_id": "5d7287fb0aa4d25606d11537dbcad1df525f669d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/22db360d53d73a76feb712ca1bda5ffbff81d29d"
        },
        "date": 1775266672461,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.633,
            "range": "± 0.013",
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
            "value": 0.073,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.276,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.778,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.532,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.02,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.494,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.395,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.191,
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
            "value": 0.086,
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.188,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.966,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.475,
            "range": "± 0.14",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.979,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 84.173,
            "range": "± 0.351",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.508,
            "range": "± 0.017",
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
          "id": "085b94cc4dffdedbc9637aa055c988e64cd85a9f",
          "message": "Fix variadic parameter element type in foreach loops",
          "timestamp": "2026-04-04T03:53:31+02:00",
          "tree_id": "477bd246a172024881ba81cab40ecbdc2129ee0a",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/085b94cc4dffdedbc9637aa055c988e64cd85a9f"
        },
        "date": 1775268144026,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.61,
            "range": "± 0.307",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.275,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.773,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.55,
            "range": "± 0.023",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
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
            "value": 3.524,
            "range": "± 0.037",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.393,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.187,
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
            "value": 0.085,
            "range": "± 0.002",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.189,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.968,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.642,
            "range": "± 0.481",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.984,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 88.435,
            "range": "± 2.036",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.511,
            "range": "± 0.039",
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
          "id": "de9fd4e424e4d825e6506e56d3f63b247bad441d",
          "message": "Infer closure param type for whereHas/whereDoesntHave relation chain",
          "timestamp": "2026-04-04T05:23:12+02:00",
          "tree_id": "4761e6963b912ab10d7955759e75d3e42febe896",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/de9fd4e424e4d825e6506e56d3f63b247bad441d"
        },
        "date": 1775273550340,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.633,
            "range": "± 0.017",
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
            "value": 0.073,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.28,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.166,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.786,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.555,
            "range": "± 0.014",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.244,
            "range": "± 0.126",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.39,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
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
            "value": 0.087,
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
            "value": 0.19,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.974,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.49,
            "range": "± 0.043",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.977,
            "range": "± 0.007",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.663,
            "range": "± 0.484",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.512,
            "range": "± 0.004",
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
          "id": "642cc626ce22dc5b4dd6c2a4200f6d87eb185286",
          "message": "Resolve @mixin template parameters via property generic types",
          "timestamp": "2026-04-04T14:41:39+02:00",
          "tree_id": "eb2196c4ecdbd8de30a20e60c4c97b119e4c4bc0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/642cc626ce22dc5b4dd6c2a4200f6d87eb185286"
        },
        "date": 1775307034828,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.622,
            "range": "± 0.011",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.276,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.781,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.536,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.021,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.253,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.401,
            "range": "± 0.012",
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
            "value": 0.088,
            "range": "± 0.003",
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
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.971,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.465,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.983,
            "range": "± 0.006",
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
            "value": 85.382,
            "range": "± 0.32",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.512,
            "range": "± 0.005",
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
          "id": "53d7967e9121114d9f006398eea08c0d5295a0a1",
          "message": "Fix diagnostic cache poisoning by depth-limited variable resolution",
          "timestamp": "2026-04-04T16:53:49+02:00",
          "tree_id": "e0dd9fd143b1cecd991e74864b2e3dbe040d5f2f",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/53d7967e9121114d9f006398eea08c0d5295a0a1"
        },
        "date": 1775314977557,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.689,
            "range": "± 0.017",
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
            "value": 0.07,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.138,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.265,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.17,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.823,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.632,
            "range": "± 0.087",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.045,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 4.049,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.372,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/short",
            "value": 0.012,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.084,
            "range": "± 0",
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
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.963,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.57,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.971,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 86.216,
            "range": "± 1.165",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.523,
            "range": "± 0.006",
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
          "id": "aa9c6b1d6ed37b45fac24360f266af4b1241f0f3",
          "message": "Update roadmap",
          "timestamp": "2026-04-04T17:07:42+02:00",
          "tree_id": "38cd853bedb138d1740054aae0ac2aa25492dd20",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/aa9c6b1d6ed37b45fac24360f266af4b1241f0f3"
        },
        "date": 1775315795835,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.617,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.072,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.281,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.768,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.521,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.417,
            "range": "± 0.052",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.386,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.202,
            "range": "± 0.002",
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
            "value": 0.088,
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.981,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.519,
            "range": "± 0.105",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.986,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 88.688,
            "range": "± 0.481",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.517,
            "range": "± 0.009",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "djsmits12@gmail.com",
            "name": "Remco Smits",
            "username": "RemcoSmitsDev"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "be5d7ebb5a29171baa9eaa354fbfa0e1d3b3d124",
          "message": "Support hover information for parameter variable\ndefinitions.\n\nThis is mainly usefull when you are working in a project with large\ndocblocks that define some types that are hard to discover, so making\nthe hover work for them allow you to easier discover the actual type of\nthe variable.",
          "timestamp": "2026-04-04T18:24:18+02:00",
          "tree_id": "1c731893e9bbecdac27b78e6f36c27e57d1ee94c",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/be5d7ebb5a29171baa9eaa354fbfa0e1d3b3d124"
        },
        "date": 1775320391099,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.617,
            "range": "± 0.029",
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
            "value": 0.072,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.148,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.279,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.796,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.577,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.378,
            "range": "± 0.04",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.386,
            "range": "± 0.01",
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
            "value": 0.088,
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
            "value": 0.188,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.967,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.467,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.979,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.803,
            "range": "± 0.638",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.519,
            "range": "± 0.017",
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
          "id": "717652bf35d7e1ef19fedd71ca64e4e58d399f42",
          "message": "Suppress false positives for Laravel Eloquent scope methods on Builder",
          "timestamp": "2026-04-04T18:39:27+02:00",
          "tree_id": "3bd75e323d58cd62f67de9de1168ebecf100402b",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/717652bf35d7e1ef19fedd71ca64e4e58d399f42"
        },
        "date": 1775321517595,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.631,
            "range": "± 0.044",
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
            "value": 0.143,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.778,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.538,
            "range": "± 0.049",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.261,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.386,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0",
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.019,
            "range": "± 0.001",
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
            "value": 0.188,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.976,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.504,
            "range": "± 0.043",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.998,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.031,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 89.326,
            "range": "± 0.384",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.512,
            "range": "± 0.007",
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
          "id": "137226f38f0e9a3cc908cdfca7e11c3146afb602",
          "message": "Add type-guard narrowing for is_array, is_object, and scalars",
          "timestamp": "2026-04-04T18:44:43+02:00",
          "tree_id": "6aeed1953700798750cc046b28175299204700b9",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/137226f38f0e9a3cc908cdfca7e11c3146afb602"
        },
        "date": 1775321654969,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.665,
            "range": "± 0.111",
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
            "value": 0.074,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.149,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.281,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.779,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.551,
            "range": "± 0.057",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0.001",
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.393,
            "range": "± 0.049",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.393,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.199,
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
            "value": 0.088,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.193,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.986,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.836,
            "range": "± 0.092",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 88.575,
            "range": "± 0.383",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.515,
            "range": "± 0.006",
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
          "id": "2693b665ea6cac86398e4fdb10027e834a5f9be9",
          "message": "Preserve generic arguments in callable param inference",
          "timestamp": "2026-04-04T21:34:34+02:00",
          "tree_id": "e4285dde6a02543983b56ab5b0009fe2c8ddd1ac",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/2693b665ea6cac86398e4fdb10027e834a5f9be9"
        },
        "date": 1775331810843,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.599,
            "range": "± 0.059",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.78,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.551,
            "range": "± 0.08",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.351,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.4,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.2,
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
            "value": 0.088,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.974,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.475,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.986,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
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
            "value": 85.472,
            "range": "± 0.218",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.514,
            "range": "± 0.006",
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
          "id": "cc071c37d84bb2c29df4379b85878d2ea0e00c6f",
          "message": "Handle void and never in is_self_like_type union filtering",
          "timestamp": "2026-04-04T22:37:14+02:00",
          "tree_id": "410c072e1c31ab8443481a80ae19edc5bcd1389e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/cc071c37d84bb2c29df4379b85878d2ea0e00c6f"
        },
        "date": 1775335563158,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.593,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.006,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.072,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.274,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.166,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.783,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.554,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.412,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.389,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.204,
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
            "value": 0.09,
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
            "value": 0.19,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.981,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.512,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.985,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.359,
            "range": "± 0.645",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.508,
            "range": "± 0.007",
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
          "id": "ab6d00e9a3dddeb656119c68cbf3d7ba53862d51",
          "message": "Clean up framework patching",
          "timestamp": "2026-04-04T23:20:11+02:00",
          "tree_id": "c4ace916e8f40d57fcc2969730227d6f854c9cce",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/ab6d00e9a3dddeb656119c68cbf3d7ba53862d51"
        },
        "date": 1775338146554,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.636,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.073,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.147,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.279,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.169,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.804,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.651,
            "range": "± 0.011",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.307,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.386,
            "range": "± 0.01",
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
            "value": 0.088,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.985,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.543,
            "range": "± 0.035",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.994,
            "range": "± 0.034",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.429,
            "range": "± 0.374",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.507,
            "range": "± 0.005",
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
          "id": "2d7f5177dae40d0e0ac6b3a64d11d6a4a1f09d11",
          "message": "Update dependencies",
          "timestamp": "2026-04-04T23:30:19+02:00",
          "tree_id": "8b703583634586037367f6517930f2ba8bd9b276",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/2d7f5177dae40d0e0ac6b3a64d11d6a4a1f09d11"
        },
        "date": 1775338747782,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.824,
            "range": "± 0.063",
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
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.274,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.169,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.799,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.586,
            "range": "± 0.016",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.309,
            "range": "± 0.055",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.394,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.195,
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
            "value": 0.087,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.018,
            "range": "± 0.001",
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
            "value": 0.191,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.982,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.481,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.989,
            "range": "± 0.021",
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
            "value": 88.457,
            "range": "± 0.325",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.513,
            "range": "± 0.01",
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
          "id": "cefa77233f88b550bc18ba1bc37a8b946c010426",
          "message": "Update roadmap",
          "timestamp": "2026-04-04T23:30:37+02:00",
          "tree_id": "6c40335d9c4683e8b15794ec4d6dacd15b827bfe",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/cefa77233f88b550bc18ba1bc37a8b946c010426"
        },
        "date": 1775338767012,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.624,
            "range": "± 0.012",
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
            "value": 0.073,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.278,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.8,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.585,
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
            "value": 0.014,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.307,
            "range": "± 0.037",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.404,
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
            "value": 0.087,
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
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.971,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.522,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.983,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 88.532,
            "range": "± 0.211",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.513,
            "range": "± 0.006",
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
          "id": "c541bac63cfaed68afb9724c3385d562d882b4a5",
          "message": "Suppress diagnostics for stdClass and object property access",
          "timestamp": "2026-04-05T00:00:36+02:00",
          "tree_id": "c39fc5e9256df165de58fe365a04fa270603a4de",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/c541bac63cfaed68afb9724c3385d562d882b4a5"
        },
        "date": 1775340561186,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.659,
            "range": "± 0.013",
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
            "value": 0.072,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.781,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.548,
            "range": "± 0.054",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.444,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.386,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
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
            "value": 0.087,
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
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.98,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.546,
            "range": "± 0.046",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.995,
            "range": "± 0.055",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
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
            "value": 89.407,
            "range": "± 5.041",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.515,
            "range": "± 0.006",
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
          "id": "a0cc936f791642d704e9da0e5e4bf3ebd4ef2960",
          "message": "Track array value types from variable-key assignments",
          "timestamp": "2026-04-05T00:30:37+02:00",
          "tree_id": "dcd4d7bcd6373805ad12f6d98a7c676e6d625f57",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a0cc936f791642d704e9da0e5e4bf3ebd4ef2960"
        },
        "date": 1775342367227,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.618,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.072,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.275,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.775,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.542,
            "range": "± 0.035",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.363,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.391,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.202,
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
            "value": 0.088,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.189,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.973,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.482,
            "range": "± 0.037",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.988,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.325,
            "range": "± 1.236",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.504,
            "range": "± 0.004",
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
          "id": "4329efea1360df71c9666c7cc2378eefea1c5fde",
          "message": "Implement bidirectional template inference from closure param types",
          "timestamp": "2026-04-05T00:56:31+02:00",
          "tree_id": "5e27993eb1ee1a1d29da938f21fb6e4032e04b8b",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4329efea1360df71c9666c7cc2378eefea1c5fde"
        },
        "date": 1775343927362,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.615,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.142,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
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
            "value": 0.775,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.541,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.341,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.39,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.212,
            "range": "± 0.005",
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
            "value": 0.091,
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
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.971,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.497,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.983,
            "range": "± 0.004",
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
            "value": 83.66,
            "range": "± 0.795",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.524,
            "range": "± 0.004",
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
          "id": "caa616349ae6842e60c8a7939e88d042ff6b6622",
          "message": "Fix class-string<T> static method dispatch and return type resolution",
          "timestamp": "2026-04-05T02:02:42+02:00",
          "tree_id": "af31b67f4753b676daf9b481ec91f475288a5129",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/caa616349ae6842e60c8a7939e88d042ff6b6622"
        },
        "date": 1775347897197,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.627,
            "range": "± 0.026",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.147,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.79,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.57,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.428,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.389,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.201,
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
            "value": 0.193,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.987,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.884,
            "range": "± 0.133",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.996,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.032,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 88.263,
            "range": "± 0.244",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.524,
            "range": "± 0.015",
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
          "id": "98883104c72845cbf5030c671f90db408c99c5ad",
          "message": "Support foreach over arrays with array shape element types",
          "timestamp": "2026-04-05T02:12:13+02:00",
          "tree_id": "8f29b1aaa50022a08d8f7ccb06423fbfb0e8f79f",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/98883104c72845cbf5030c671f90db408c99c5ad"
        },
        "date": 1775348458731,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.612,
            "range": "± 0.02",
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
            "value": 0.073,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
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
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.777,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.539,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.051,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.407,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.392,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.02,
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
            "value": 0.088,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.02,
            "range": "± 0.001",
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
            "value": 0.192,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.984,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.626,
            "range": "± 0.053",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.001,
            "range": "± 0.044",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 87.811,
            "range": "± 0.638",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.514,
            "range": "± 0.005",
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
          "id": "eafea0bc92928c76276c9964658aaa40ad78afb0",
          "message": "Update bug list",
          "timestamp": "2026-04-05T02:43:01+02:00",
          "tree_id": "c7fa82e51d1a384b34da3b9cd07a811fe180b153",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/eafea0bc92928c76276c9964658aaa40ad78afb0"
        },
        "date": 1775350308941,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.636,
            "range": "± 0.019",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.781,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.543,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.463,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.395,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.202,
            "range": "± 0.003",
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
            "range": "± 0.002",
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
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.972,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.492,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.984,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.031,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 88.024,
            "range": "± 0.544",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.514,
            "range": "± 0.004",
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
          "id": "9ede72e4d5b7148b5c02f34ce1f9ebbe10cb010c",
          "message": "Patch DB select return types and Redis mixin for Laravel",
          "timestamp": "2026-04-05T03:06:38+02:00",
          "tree_id": "890399462da6b9e1f3f793ef5a5a9403783e02db",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/9ede72e4d5b7148b5c02f34ce1f9ebbe10cb010c"
        },
        "date": 1775351725325,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.603,
            "range": "± 0.009",
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
            "value": 0.074,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.148,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.277,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.174,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.826,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.636,
            "range": "± 0.016",
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
            "value": 0.021,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.319,
            "range": "± 0.219",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.383,
            "range": "± 0.01",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.087,
            "range": "± 0",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.988,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.565,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.996,
            "range": "± 0.012",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 86.588,
            "range": "± 3.582",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.52,
            "range": "± 0.004",
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
          "id": "7230d35149bc1c23d3ae0a6b43781615e437fb28",
          "message": "Fix in_array guard clause type narrowing and class-string<T> static\nreturn",
          "timestamp": "2026-04-05T04:19:24+02:00",
          "tree_id": "5f868a3d4e0ce5c954c3907e271df18260ef2974",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/7230d35149bc1c23d3ae0a6b43781615e437fb28"
        },
        "date": 1775356195646,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.622,
            "range": "± 0.007",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.273,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.776,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.536,
            "range": "± 0.013",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.235,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.39,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.196,
            "range": "± 0.007",
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
            "value": 0.088,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.98,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.509,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.991,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 87.355,
            "range": "± 0.515",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.503,
            "range": "± 0.013",
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
          "id": "108c175384da3eadf05f7a0b81852d3a8eead9a5",
          "message": "Fix `instanceof` narrowing with unresolvable target class",
          "timestamp": "2026-04-05T15:03:40+02:00",
          "tree_id": "1462922b33dfeee84d21adaaf2dc8632c1817af9",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/108c175384da3eadf05f7a0b81852d3a8eead9a5"
        },
        "date": 1775394742447,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.631,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.07,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.142,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.262,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.764,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.511,
            "range": "± 0.025",
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
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
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
            "value": 3.352,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.41,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.201,
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
            "value": 0.088,
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
            "value": 0.188,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.975,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.487,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.992,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.031,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 60.858,
            "range": "± 0.213",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.543,
            "range": "± 0.007",
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
          "id": "dec5a3eaf0e69e0b39c20ed1bd105930767d6f96",
          "message": "Fix array shape tracking from keyed assignments in conditional loops",
          "timestamp": "2026-04-05T18:31:05+02:00",
          "tree_id": "c030da89f008f12ab77be74487e32476fa0290a9",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/dec5a3eaf0e69e0b39c20ed1bd105930767d6f96"
        },
        "date": 1775407215699,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.992,
            "range": "± 0.101",
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
            "value": 0.07,
            "range": "± 0.001",
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
            "value": 0.27,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.779,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.549,
            "range": "± 0.025",
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.31,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.391,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.196,
            "range": "± 0.007",
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
            "value": 0.087,
            "range": "± 0",
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
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.974,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.581,
            "range": "± 0.298",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.987,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
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
            "value": 61.512,
            "range": "± 0.336",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.55,
            "range": "± 0.009",
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
          "id": "31cf3e464612ca4716ab4954f7bfbbdfac0328e1",
          "message": "Implement \"Remove unused return type\" code action for PHPStan, and\nupdate general docs",
          "timestamp": "2026-04-05T20:28:21+02:00",
          "tree_id": "88847ca4e5bfc01a48a600e6a61d2a3d99a5a651",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/31cf3e464612ca4716ab4954f7bfbbdfac0328e1"
        },
        "date": 1775414240805,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.858,
            "range": "± 0.061",
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
            "value": 0.072,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.002",
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
            "value": 0.171,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.803,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.593,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.002",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.507,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.386,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.191,
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
            "value": 0.087,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.189,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.977,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.51,
            "range": "± 0.059",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.995,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 61.547,
            "range": "± 0.285",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.544,
            "range": "± 0.009",
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
          "id": "fe446e817fc437d7a5ec319fce3189c972a1d322",
          "message": "Update roadmap",
          "timestamp": "2026-04-05T22:10:22+02:00",
          "tree_id": "b6cb673946bb2764ab3972b153186dec108b018e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/fe446e817fc437d7a5ec319fce3189c972a1d322"
        },
        "date": 1775420352655,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.575,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.069,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.139,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.253,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.157,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.77,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.546,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.628,
            "range": "± 0.079",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.372,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0",
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
            "value": 0.083,
            "range": "± 0",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.176,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.955,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.466,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.962,
            "range": "± 0.004",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.757,
            "range": "± 0.4",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.507,
            "range": "± 0.011",
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
          "id": "401cb6518e0a8f203b7cb5fc958e8b33477a7e89",
          "message": "Update docs",
          "timestamp": "2026-04-06T01:24:25+02:00",
          "tree_id": "229cc1960beee8527fb8d5fd423d3c7c1f0874f9",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/401cb6518e0a8f203b7cb5fc958e8b33477a7e89"
        },
        "date": 1775431999842,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.61,
            "range": "± 0.047",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.072,
            "range": "± 0.001",
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
            "value": 0.263,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.77,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.533,
            "range": "± 0.022",
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
            "value": 0.01,
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
            "value": 3.579,
            "range": "± 0.04",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.396,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.189,
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
            "value": 0.187,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.964,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.46,
            "range": "± 0.044",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.976,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.37,
            "range": "± 0.224",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.533,
            "range": "± 0.006",
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
          "id": "c3638ce70e321df3a184015f7af6c2f8a1b6f5bc",
          "message": "Support go-to-definition for @see tags in floating docblocks",
          "timestamp": "2026-04-06T01:50:39+02:00",
          "tree_id": "6e277120d39743a684cd83d1d12a552c12b53e17",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/c3638ce70e321df3a184015f7af6c2f8a1b6f5bc"
        },
        "date": 1775433550246,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.641,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.775,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.557,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
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
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.374,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.394,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.202,
            "range": "± 0.01",
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
            "value": 0.09,
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
            "value": 0.201,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.034,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.742,
            "range": "± 0.173",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.038,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.564,
            "range": "± 0.268",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.547,
            "range": "± 0.006",
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
          "id": "173d25219dab4b4cc7891c43e3b6b056d54584c4",
          "message": "Fix false unused import diagnostic for aliased namespace used in\nattributes",
          "timestamp": "2026-04-06T02:14:26+02:00",
          "tree_id": "d98d9a40ff22669d4c344c40ce469dad42cf1e0c",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/173d25219dab4b4cc7891c43e3b6b056d54584c4"
        },
        "date": 1775434986965,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.623,
            "range": "± 0.024",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.145,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.166,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.779,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.547,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.019,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.533,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.404,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.188,
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
            "value": 0.086,
            "range": "± 0",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.201,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.044,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.868,
            "range": "± 0.056",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.059,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.536,
            "range": "± 0.236",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.543,
            "range": "± 0.005",
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
          "id": "410f8c410b5e04ce11c0fe62a245d57c093622cb",
          "message": "Update roadmap",
          "timestamp": "2026-04-06T02:48:43+02:00",
          "tree_id": "d085dff638b73d8188bb401df2d054ffaf6f4e6c",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/410f8c410b5e04ce11c0fe62a245d57c093622cb"
        },
        "date": 1775437047143,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.616,
            "range": "± 0.015",
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
            "range": "± 0",
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
            "value": 0.265,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.777,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.546,
            "range": "± 0.012",
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
            "value": 3.537,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.401,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.189,
            "range": "± 0.002",
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
            "value": 0.086,
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.026,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.808,
            "range": "± 0.042",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.047,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.84,
            "range": "± 0.801",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.543,
            "range": "± 0.008",
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
          "id": "a3546295347725110a95fbc07a52c04ddeca7dab",
          "message": "Fix diagnostics, import actions, and references for global classes",
          "timestamp": "2026-04-06T02:52:44+02:00",
          "tree_id": "01c3e70cca184ba96ebb0a9e55e7523dafe48b59",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a3546295347725110a95fbc07a52c04ddeca7dab"
        },
        "date": 1775437302726,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.952,
            "range": "± 0.286",
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
            "value": 0.073,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.783,
            "range": "± 0.081",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.573,
            "range": "± 0.055",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.002",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.199,
            "range": "± 0.091",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.398,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.191,
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
            "value": 0.087,
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
            "value": 0.202,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.041,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.266,
            "range": "± 0.194",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.049,
            "range": "± 0.014",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.727,
            "range": "± 0.902",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.548,
            "range": "± 0.02",
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
          "id": "dd2265d1b854ef2cd258e0760a20e9fb0471d2d3",
          "message": "Migrate bracket-access segment walks to use PhpType throughout",
          "timestamp": "2026-04-06T03:03:01+02:00",
          "tree_id": "ff4471dd5fb283f9288e1ce42d3e2b517b56c3eb",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/dd2265d1b854ef2cd258e0760a20e9fb0471d2d3"
        },
        "date": 1775437890857,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.612,
            "range": "± 0.011",
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
            "value": 0.07,
            "range": "± 0.001",
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
            "value": 0.263,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.79,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.577,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
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
            "value": 3.311,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.389,
            "range": "± 0.011",
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
            "value": 0.018,
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
            "value": 0.088,
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
            "value": 0.013,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.203,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.056,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.868,
            "range": "± 0.077",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.038,
            "range": "± 0.023",
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
            "value": 61.4,
            "range": "± 0.329",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.54,
            "range": "± 0.01",
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
          "id": "5dfc4562fa4242b67c749c7ce369b9e63f29fc4e",
          "message": "Accept &[PhpType] in generic-arg resolution",
          "timestamp": "2026-04-06T03:06:17+02:00",
          "tree_id": "b0708709bbb326d58dd19e70d4c6496475fa7d85",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5dfc4562fa4242b67c749c7ce369b9e63f29fc4e"
        },
        "date": 1775438172130,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.573,
            "range": "± 0.034",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.068,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.136,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.246,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.153,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.752,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.497,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.044,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 3.486,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.377,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.189,
            "range": "± 0",
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
            "value": 0.083,
            "range": "± 0",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.184,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.997,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.665,
            "range": "± 0.19",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.013,
            "range": "± 0.02",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 58.452,
            "range": "± 0.175",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.518,
            "range": "± 0.014",
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
          "id": "5d4f2a01303e71566f6333c8087de79673cabd4d",
          "message": "Substituted return types as PhpType",
          "timestamp": "2026-04-06T03:12:55+02:00",
          "tree_id": "7f539a7d8f264d57d6537a17540b5fadfc6ea651",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5d4f2a01303e71566f6333c8087de79673cabd4d"
        },
        "date": 1775438511958,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.763,
            "range": "± 0.123",
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
            "value": 0.073,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.001",
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
            "value": 0.169,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.796,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.585,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0.001",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.261,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.391,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.195,
            "range": "± 0.003",
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
            "value": 0.088,
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
            "value": 0.2,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.029,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.919,
            "range": "± 0.197",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.044,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.028,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.558,
            "range": "± 0.298",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.548,
            "range": "± 0.008",
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
          "id": "f4e410a88fd575694d273ae60fb27e2199bc7738",
          "message": "Replace type_strings_joined with types_joined and to_string",
          "timestamp": "2026-04-06T03:21:24+02:00",
          "tree_id": "cfd3fd269b5754fc287c2221eaae3f90d8eac0d7",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/f4e410a88fd575694d273ae60fb27e2199bc7738"
        },
        "date": 1775439015003,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.604,
            "range": "± 0.05",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
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
            "value": 0.145,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.777,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.543,
            "range": "± 0.042",
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
            "value": 3.503,
            "range": "± 0.133",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.4,
            "range": "± 0.008",
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
            "value": 0.012,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.086,
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
            "value": 0.194,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.005,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.704,
            "range": "± 0.071",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.031,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.636,
            "range": "± 0.321",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.542,
            "range": "± 0.008",
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
          "id": "481ac8b299bc9e8afccde1ecbb2c662b350a881e",
          "message": "Refactor type alias expansion to return PhpType instead of String",
          "timestamp": "2026-04-06T03:27:21+02:00",
          "tree_id": "3b7051a79040b263e5af9b99be3ec797e64bbbc0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/481ac8b299bc9e8afccde1ecbb2c662b350a881e"
        },
        "date": 1775439354550,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.606,
            "range": "± 0.012",
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
            "value": 0.143,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.78,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.548,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.472,
            "range": "± 0.034",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.399,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.204,
            "range": "± 0.003",
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
            "value": 0.013,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.199,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.015,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.664,
            "range": "± 0.067",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.034,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.395,
            "range": "± 0.881",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.543,
            "range": "± 0.006",
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
          "id": "179b554f76820066a7d90aa349eafc00481c966c",
          "message": "Update roadmap",
          "timestamp": "2026-04-06T03:34:23+02:00",
          "tree_id": "868a5789babd7b99f4ebac4696d77263bb3f1da9",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/179b554f76820066a7d90aa349eafc00481c966c"
        },
        "date": 1775439773233,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.625,
            "range": "± 0.006",
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
            "value": 0.142,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.774,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.53,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.409,
            "range": "± 0.034",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.396,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.204,
            "range": "± 0.002",
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
            "value": 0.202,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.032,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.727,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.044,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 61.461,
            "range": "± 0.367",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.547,
            "range": "± 0.01",
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
          "id": "7e30105f6490f5d779e8c5b9515f3c6b692a8e40",
          "message": "Add structural subtype and type simplification methods to PhpType",
          "timestamp": "2026-04-06T03:55:08+02:00",
          "tree_id": "75cb5029e282800d276247e23b0251dd808a5623",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/7e30105f6490f5d779e8c5b9515f3c6b692a8e40"
        },
        "date": 1775441022505,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.63,
            "range": "± 0.018",
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
            "value": 0.072,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.79,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.564,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.252,
            "range": "± 0.215",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.393,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.018,
            "range": "± 0.001",
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
            "value": 0.198,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.016,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.648,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.032,
            "range": "± 0.034",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 60.311,
            "range": "± 0.464",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.539,
            "range": "± 0.006",
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
          "id": "d8289587566f931352f17c5da9297678492b9249",
          "message": "Switch to type_hint_to_classes_typed with pre-parsed PhpType",
          "timestamp": "2026-04-06T03:55:34+02:00",
          "tree_id": "e459f9a5ebf257cc42dc5aacb0515a8ee7ceed1d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d8289587566f931352f17c5da9297678492b9249"
        },
        "date": 1775441050900,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.614,
            "range": "± 0.059",
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
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.759,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.509,
            "range": "± 0.012",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.416,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.393,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.197,
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
            "value": 0.088,
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
            "value": 0.201,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.042,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.795,
            "range": "± 0.042",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.051,
            "range": "± 0.017",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.869,
            "range": "± 0.466",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.539,
            "range": "± 0.006",
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
          "id": "4b04d527b826b17994efb645db0f1aa7e119d6ea",
          "message": "Refactor array/object shape entry types to use ShapeEntry",
          "timestamp": "2026-04-06T03:58:20+02:00",
          "tree_id": "4df64fee507269d8f04a07ed5af4e0d90efe8634",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4b04d527b826b17994efb645db0f1aa7e119d6ea"
        },
        "date": 1775441374609,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.696,
            "range": "± 0.055",
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
            "value": 0.146,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.166,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.786,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.558,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.019,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.373,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.407,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.199,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.088,
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
            "value": 0.199,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.023,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.766,
            "range": "± 0.101",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.041,
            "range": "± 0.014",
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
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 63.034,
            "range": "± 0.361",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.547,
            "range": "± 0.006",
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
          "id": "6f3919e8abf1bba3132f7e1f4a8de523c577e3a9",
          "message": "Remove clean_type utility and update docblock parsing to preserve type\nstrings",
          "timestamp": "2026-04-06T04:08:47+02:00",
          "tree_id": "1f136da63ffaf2d20bf924e94945334a6f5900a0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6f3919e8abf1bba3132f7e1f4a8de523c577e3a9"
        },
        "date": 1775441849588,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.605,
            "range": "± 0.039",
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
            "value": 0.143,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.794,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.571,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.002",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.36,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.404,
            "range": "± 0.011",
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
            "value": 0.087,
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
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.022,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.677,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.042,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 61.019,
            "range": "± 0.92",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.549,
            "range": "± 0.005",
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
          "id": "60c65da14992db76a2756837245ab38e564f4697",
          "message": "Migrate type override resolution to accept parsed types",
          "timestamp": "2026-04-06T04:15:10+02:00",
          "tree_id": "a6739728b14b00c8e8f9ddd2e868fd8676cdf130",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/60c65da14992db76a2756837245ab38e564f4697"
        },
        "date": 1775442230628,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.528,
            "range": "± 0.007",
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
            "value": 0.072,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.147,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.169,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.802,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.588,
            "range": "± 0.025",
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
            "value": 0.014,
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.317,
            "range": "± 0.299",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.409,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.193,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.087,
            "range": "± 0",
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
            "value": 0.193,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.004,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.608,
            "range": "± 0.036",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.012,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.923,
            "range": "± 1.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.542,
            "range": "± 0.006",
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
          "id": "dd53547e246738f2aca6df7ddd03ae53a9a7aa87",
          "message": "Migrate enrichment functions to accept PhpType and update all call sites",
          "timestamp": "2026-04-06T04:21:55+02:00",
          "tree_id": "92be153cd5e63ede00fa1344e0e05692827567d2",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/dd53547e246738f2aca6df7ddd03ae53a9a7aa87"
        },
        "date": 1775442655776,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.519,
            "range": "± 0.03",
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
            "value": 0.143,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.268,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.166,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.783,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.558,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 3.355,
            "range": "± 0.068",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.396,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.188,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.082,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.187,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.999,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.615,
            "range": "± 0.061",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.002,
            "range": "± 0.029",
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
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.624,
            "range": "± 1.442",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.517,
            "range": "± 0.014",
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
          "id": "ee9b87dcd1e4b50af9e73337fe3f0333cc74068b",
          "message": "please rebase on origin",
          "timestamp": "2026-04-06T04:27:42+02:00",
          "tree_id": "c97e81f44073d6bfb365a39f2c03f59516f89c10",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/ee9b87dcd1e4b50af9e73337fe3f0333cc74068b"
        },
        "date": 1775442974476,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.521,
            "range": "± 0.037",
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
            "value": 0.07,
            "range": "± 0.001",
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
            "value": 0.266,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.756,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.504,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.303,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.402,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.197,
            "range": "± 0.007",
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
            "value": 0.088,
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
            "value": 0.193,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.006,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.625,
            "range": "± 0.2",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.023,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 62.151,
            "range": "± 0.751",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.544,
            "range": "± 0.011",
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
          "id": "a084610fac917e6c9eca0db29ebd2f9282406e47",
          "message": "Switch MethodInfo and FunctionInfo throws fields to Vec<PhpType>",
          "timestamp": "2026-04-06T04:29:48+02:00",
          "tree_id": "f06eefb5850634ed7754ddea7a0a3027d0647e65",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a084610fac917e6c9eca0db29ebd2f9282406e47"
        },
        "date": 1775449216919,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.484,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.067,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.136,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.247,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.158,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.776,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.551,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.045,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 3.576,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.371,
            "range": "± 0.007",
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
            "value": 0.018,
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
            "value": 0.083,
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
            "value": 0.181,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.982,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.525,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.997,
            "range": "± 0.021",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 58.163,
            "range": "± 0.716",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.506,
            "range": "± 0.007",
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
          "id": "852b4b68f6f7b05d03661d678d03d3409fb5fe6d",
          "message": "Update roadmap",
          "timestamp": "2026-04-06T06:33:15+02:00",
          "tree_id": "ea579a385686e285fc1c01225acd967f54e661f1",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/852b4b68f6f7b05d03661d678d03d3409fb5fe6d"
        },
        "date": 1775450505968,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.533,
            "range": "± 0.017",
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
            "value": 0.072,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.262,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.814,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.532,
            "range": "± 0.014",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.355,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.397,
            "range": "± 0.009",
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
            "value": 0.088,
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
            "value": 0.192,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.997,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.626,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.012,
            "range": "± 0.018",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.73,
            "range": "± 0.197",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.535,
            "range": "± 0.007",
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
          "id": "bf0ac3ae1dca35696be7b4e194546a61de37b872",
          "message": "Recognize @psalm-return, @psalm-param, and @psalm-var tags",
          "timestamp": "2026-04-06T06:55:47+02:00",
          "tree_id": "c740e8a7b828d9fe04368aa74dd3da08aa482069",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/bf0ac3ae1dca35696be7b4e194546a61de37b872"
        },
        "date": 1775451880297,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.762,
            "range": "± 0.063",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.067,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.135,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.25,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.154,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.753,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.507,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.044,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
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
            "value": 3.502,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.371,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.191,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.083,
            "range": "± 0",
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
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.981,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.628,
            "range": "± 0.054",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.002,
            "range": "± 0.01",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 58.075,
            "range": "± 0.161",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.514,
            "range": "± 0.013",
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
          "id": "7a3f8b16c0b6da005b3d43aa872c1e6d4e926a9d",
          "message": "Integrate PhpType directly for native hints and conditional types",
          "timestamp": "2026-04-06T07:42:49+02:00",
          "tree_id": "e25548666d8f0cdddcf96063ffd2a6c95d7fb97c",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/7a3f8b16c0b6da005b3d43aa872c1e6d4e926a9d"
        },
        "date": 1775454687360,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.54,
            "range": "± 0.132",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.13,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.069",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.766,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.523,
            "range": "± 0.199",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.313,
            "range": "± 0.113",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.349,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/short",
            "value": 0.012,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.088,
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
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.931,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.304,
            "range": "± 0.166",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.95,
            "range": "± 0.004",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.583,
            "range": "± 0.256",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.536,
            "range": "± 0.005",
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
          "id": "6e45044d3d2b03a79f45291d7cd5c2882555ebe2",
          "message": "Fix cross-file property type resolution for promoted and self types",
          "timestamp": "2026-04-06T08:04:59+02:00",
          "tree_id": "c64195d7d66b1a9f89ec60c9d0946eeeed949066",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6e45044d3d2b03a79f45291d7cd5c2882555ebe2"
        },
        "date": 1775456020188,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.527,
            "range": "± 0.011",
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
            "value": 0.066,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.278,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.766,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.522,
            "range": "± 0.02",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.406,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.351,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.087,
            "range": "± 0.002",
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
            "value": 0.183,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.947,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.405,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.955,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 59.813,
            "range": "± 2.293",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.547,
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
          "id": "ef8ceffaaf295edf42139ecad3ef40340eda283b",
          "message": "Update roadmap.",
          "timestamp": "2026-04-06T09:17:25+02:00",
          "tree_id": "b122acafb253be18c68ffd5351e00810d691b5b3",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/ef8ceffaaf295edf42139ecad3ef40340eda283b"
        },
        "date": 1775460370982,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.522,
            "range": "± 0.067",
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
            "value": 0.066,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.276,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.765,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.518,
            "range": "± 0.039",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
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
            "value": 3.352,
            "range": "± 0.095",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.356,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.018,
            "range": "± 0.001",
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
            "value": 0.183,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.955,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.42,
            "range": "± 0.073",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.961,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 59.899,
            "range": "± 0.465",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.547,
            "range": "± 0.005",
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
          "id": "a4b8226f5f0c21638ddc2c2ee4a77b81946c691c",
          "message": "Refactor variable type resolution to use PhpType throughout relationship\nhelpers",
          "timestamp": "2026-04-06T16:28:40+02:00",
          "tree_id": "b394fab23f23e5d96549eff3c7b7b7f005740791",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a4b8226f5f0c21638ddc2c2ee4a77b81946c691c"
        },
        "date": 1775486251853,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.497,
            "range": "± 0.01",
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.763,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.518,
            "range": "± 0.017",
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
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.259,
            "range": "± 0.053",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.338,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.087,
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
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.182,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.941,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.338,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.953,
            "range": "± 0.003",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.344,
            "range": "± 0.194",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.535,
            "range": "± 0.003",
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
          "id": "37a901ed4864f113b53d5013b36ad18ca056780d",
          "message": "Fix class-string<T> static method dispatch and return type resolution",
          "timestamp": "2026-04-06T19:46:38+02:00",
          "tree_id": "963c5b8bcc188dcede2f4bfcf53013119f6afe52",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/37a901ed4864f113b53d5013b36ad18ca056780d"
        },
        "date": 1775498131980,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.539,
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.778,
            "range": "± 0.063",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.547,
            "range": "± 0.069",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.492,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.34,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.188,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.181,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.946,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.367,
            "range": "± 0.046",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.971,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.04,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.038,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.999,
            "range": "± 0.307",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.544,
            "range": "± 0.009",
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
          "id": "9a926e00e91639a444a997e4de779c7c558f165e",
          "message": "Add issue templates",
          "timestamp": "2026-04-06T23:54:10+02:00",
          "tree_id": "e4a1d67cdcb5bcfa3592d8670a78fb50924470ef",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/9a926e00e91639a444a997e4de779c7c558f165e"
        },
        "date": 1775512989460,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.532,
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
            "value": 0.066,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.769,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.526,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.488,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.348,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
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
            "value": 0.183,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.946,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.356,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.956,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.04,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.038,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.609,
            "range": "± 1.608",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.543,
            "range": "± 0.006",
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
          "id": "40eeffe0ffb24a4351626e22b718e9b43aceb038",
          "message": "Add issue templates",
          "timestamp": "2026-04-06T23:55:11+02:00",
          "tree_id": "1152ef7c983591e9c16f0e1b155a8916f7cf9dbc",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/40eeffe0ffb24a4351626e22b718e9b43aceb038"
        },
        "date": 1775513031624,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.523,
            "range": "± 0.016",
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
            "value": 0.066,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.774,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.506,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.462,
            "range": "± 0.158",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.34,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.187,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.086,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
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
            "value": 0.182,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.943,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.57,
            "range": "± 0.122",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.949,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.04,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.038,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.563,
            "range": "± 0.29",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.534,
            "range": "± 0.009",
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
          "id": "c85d53071a99662df105f520c2de99bdee89df1b",
          "message": "Fix docblock completion for multibyte characters in UTF-16",
          "timestamp": "2026-04-07T00:25:26+02:00",
          "tree_id": "19b39f67012f989e57a028584cdeb628cbc47a6b",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/c85d53071a99662df105f520c2de99bdee89df1b"
        },
        "date": 1775514852764,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.529,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.066,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.276,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.002",
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
            "value": 1.563,
            "range": "± 0.021",
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
            "value": 0.016,
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
            "value": 3.594,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.346,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.2,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.017,
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
            "value": 0.184,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.952,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.43,
            "range": "± 0.033",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.961,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.04,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.038,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.406,
            "range": "± 1.934",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.531,
            "range": "± 0.007",
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
          "id": "d7626f2c5fd5ca086f1300829abf21b78c958a62",
          "message": "Migrate all type-string manipulation to structured PhpType",
          "timestamp": "2026-04-07T01:40:19+02:00",
          "tree_id": "0e2eca7ab331cd6a3cb4f079056fc01c2fe88b3a",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d7626f2c5fd5ca086f1300829abf21b78c958a62"
        },
        "date": 1775519341572,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.504,
            "range": "± 0.016",
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
            "value": 0.066,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.77,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.535,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.276,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.355,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
            "range": "± 0.001",
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
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
            "value": 0.179,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.93,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.306,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.941,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 62.835,
            "range": "± 0.237",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.528,
            "range": "± 0.008",
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
          "id": "b9af98ed9ff6fac879629445b17638a28ca5d5e5",
          "message": "Recognize FQN \\assert() as narrowing for instanceof checks",
          "timestamp": "2026-04-07T02:24:49+02:00",
          "tree_id": "63ce813ea7dd93fc34af911bc6abcd0cb417743a",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b9af98ed9ff6fac879629445b17638a28ca5d5e5"
        },
        "date": 1775522012537,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.518,
            "range": "± 0.01",
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
            "value": 0.067,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.274,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.784,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.563,
            "range": "± 0.011",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.274,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.352,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.196,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.088,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0.001",
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
            "value": 0.181,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.931,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.301,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.94,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 61.632,
            "range": "± 0.483",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.539,
            "range": "± 0.006",
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
          "id": "dc40157b2c2dc207912dbec1d6ad44d14c0c339d",
          "message": "Resolve foreach element type for union types containing arrays",
          "timestamp": "2026-04-07T02:48:37+02:00",
          "tree_id": "ba81ae0ac5e6f533bee885081934f1c8d8243f89",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/dc40157b2c2dc207912dbec1d6ad44d14c0c339d"
        },
        "date": 1775523443360,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.555,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.134,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.003",
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
            "value": 0.785,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.561,
            "range": "± 0.026",
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
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.34,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.344,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.017,
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
            "value": 0.183,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.945,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.383,
            "range": "± 0.049",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.962,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 62.756,
            "range": "± 0.326",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.538,
            "range": "± 0.007",
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
          "id": "e3fa35b3b4411791114ed66cf0cbeef27b18f07a",
          "message": "Refactor PhpType handling and template defaults to use typed values",
          "timestamp": "2026-04-07T03:05:20+02:00",
          "tree_id": "6c20e38c6f5790aee7bc82f4ef5e9ebf637c4bd4",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/e3fa35b3b4411791114ed66cf0cbeef27b18f07a"
        },
        "date": 1775524444740,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.515,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.066,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.136,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.274,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.794,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.586,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0.001",
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.428,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.349,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0.002",
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
            "value": 0.177,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.929,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.362,
            "range": "± 0.066",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.945,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.783,
            "range": "± 0.543",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.525,
            "range": "± 0.007",
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
          "id": "01b0467d5f646f4b40264b3223e6760d9606cdf9",
          "message": "Update roadmap",
          "timestamp": "2026-04-07T06:09:08+02:00",
          "tree_id": "5fa95238ac9dc5f5b8d18c76462edbabd427222f",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/01b0467d5f646f4b40264b3223e6760d9606cdf9"
        },
        "date": 1775535480301,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.516,
            "range": "± 0.013",
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.134,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.769,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.532,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.51,
            "range": "± 0.065",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.345,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
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
            "value": 0.182,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.938,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.361,
            "range": "± 0.033",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.942,
            "range": "± 0.004",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 60.333,
            "range": "± 0.391",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.004",
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
          "id": "9f539f9238782772a2b5c51e37499305eb885e2d",
          "message": "Document LSP client/editor integration, update todo, and improve type\nhandling",
          "timestamp": "2026-04-07T06:48:28+02:00",
          "tree_id": "09804016c6499f427a1787d91cf2259a2ef85f00",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/9f539f9238782772a2b5c51e37499305eb885e2d"
        },
        "date": 1775537831954,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.508,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.065,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.783,
            "range": "± 0.047",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.523,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0.001",
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.413,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.351,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
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
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.934,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.356,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.948,
            "range": "± 0.009",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.628,
            "range": "± 0.256",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.006",
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
          "id": "07cff1ffca1a29645c974b676959affe361a2937",
          "message": "Integrate PhpType throughout completion, code actions, and virtual\nmembers",
          "timestamp": "2026-04-07T07:16:25+02:00",
          "tree_id": "2a0a6755abc2b16f21b38721cb128c1ce91acf05",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/07cff1ffca1a29645c974b676959affe361a2937"
        },
        "date": 1775539526947,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.548,
            "range": "± 0.057",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.068,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.136,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.278,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.771,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.54,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.048,
            "range": "± 0.001",
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
            "value": 3.683,
            "range": "± 0.038",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.348,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
            "range": "± 0.001",
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
            "value": 0.088,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
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
            "value": 0.184,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.96,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.451,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.972,
            "range": "± 0.008",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 64.576,
            "range": "± 0.839",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.555,
            "range": "± 0.007",
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
          "id": "533efc5042fe95f57ee7869229538d0931e69d32",
          "message": "Fix undefined variable false positives in nested scopes",
          "timestamp": "2026-04-08T02:31:09+02:00",
          "tree_id": "3c8f62bc7bdb56a66a86fbc4e02e3cb136f91196",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/533efc5042fe95f57ee7869229538d0931e69d32"
        },
        "date": 1775608797953,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.498,
            "range": "± 0.052",
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.132,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.276,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.759,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.503,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.481,
            "range": "± 0.274",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.339,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.189,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
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
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.92,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.276,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.942,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 60.702,
            "range": "± 0.47",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.003",
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
          "id": "7a2c0f2df28d09dae3fe64c4627fa82d20c99a83",
          "message": "Fix docblock return enrichment to detect existing type structure",
          "timestamp": "2026-04-08T02:47:32+02:00",
          "tree_id": "c8bf7f204bb512e216cba532118592736610f1df",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/7a2c0f2df28d09dae3fe64c4627fa82d20c99a83"
        },
        "date": 1775609846614,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.504,
            "range": "± 0.039",
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.129,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.771,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.531,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.362,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.34,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.197,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.088,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
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
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.924,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.349,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.937,
            "range": "± 0.012",
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
            "value": 61.981,
            "range": "± 0.447",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.522,
            "range": "± 0.004",
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
          "id": "5568602924b43a6ff6d92d27d4023cecd27b9ec5",
          "message": "Preserve intersection and generic types through resolution",
          "timestamp": "2026-04-08T03:17:37+02:00",
          "tree_id": "8d609d991769eed24936728afec31f8138bf3e00",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5568602924b43a6ff6d92d27d4023cecd27b9ec5"
        },
        "date": 1775611584515,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.45,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.06,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.122,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.249,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.153,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.754,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.51,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.042,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.438,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.316,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
            "range": "± 0.001",
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
            "value": 0.083,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.167,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.911,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.349,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.921,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 57.401,
            "range": "± 1.359",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.492,
            "range": "± 0.005",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "lucasalvcoutinho@gmail.com",
            "name": "Lucas Coutinho",
            "username": "lucasacoutinho"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "c0ba452a5455e95b624b599e1d2444d6d4c0e6b5",
          "message": "Fix false-positive undefined variable on static property access",
          "timestamp": "2026-04-08T03:25:54+02:00",
          "tree_id": "0ed904489274f76008c217aca0530b0f6347ebaa",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/c0ba452a5455e95b624b599e1d2444d6d4c0e6b5"
        },
        "date": 1775612089457,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.553,
            "range": "± 0.061",
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
            "value": 0.065,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.134,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.777,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.545,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.383,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.343,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.2,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.016,
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
            "value": 0.181,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.941,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.475,
            "range": "± 0.1",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.955,
            "range": "± 0.006",
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
            "value": 59.539,
            "range": "± 1.487",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.529,
            "range": "± 0.01",
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
          "id": "8559c6e887fb51572b4f1a49ab97373303aa5e50",
          "message": "Add --tcp flag to run LSP server over TCP socket",
          "timestamp": "2026-04-08T03:47:45+02:00",
          "tree_id": "4ed858f6e9b4f699307d95af64dcba4573c30129",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/8559c6e887fb51572b4f1a49ab97373303aa5e50"
        },
        "date": 1775613410771,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.489,
            "range": "± 0.013",
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
            "value": 0.064,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.13,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.769,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.518,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.412,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.351,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.199,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.18,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.93,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.348,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.941,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.781,
            "range": "± 0.395",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.011",
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
          "id": "1f48613a68e53186bf2438ab8a3936a8ca0b1f44",
          "message": "Fix completion on `<?php` open tag and chained static call parent\nhandling",
          "timestamp": "2026-04-08T04:42:12+02:00",
          "tree_id": "be226cb84fe628191063360240464b2bdfbfc147",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/1f48613a68e53186bf2438ab8a3936a8ca0b1f44"
        },
        "date": 1775616677705,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.473,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.063,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.127,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.252,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.157,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.77,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.539,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.044,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.441,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.312,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.083,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.166,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.905,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.287,
            "range": "± 0.038",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.923,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 57.785,
            "range": "± 0.38",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.499,
            "range": "± 0.01",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "lucasalvcoutinho@gmail.com",
            "name": "Lucas Coutinho",
            "username": "lucasacoutinho"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "79a50251b866a2e0325450a5f63ca13ed8a116e4",
          "message": "feat: Return self-location at definition sites so editors can fall back\nto Find References",
          "timestamp": "2026-04-08T05:37:32+02:00",
          "tree_id": "97d8965c7c179498d420697d3aa31d70cf09c753",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/79a50251b866a2e0325450a5f63ca13ed8a116e4"
        },
        "date": 1775619981113,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.5,
            "range": "± 0.052",
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
            "value": 0.064,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.266,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.765,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.507,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 3.394,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.336,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.186,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.18,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.934,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.395,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.954,
            "range": "± 0.006",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 59.454,
            "range": "± 0.305",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.532,
            "range": "± 0.005",
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
          "id": "31aaf2ea1f24f481ea8307c07ea37d6d7d189688",
          "message": "Refactor PhpType usage to prefer structured types over strings",
          "timestamp": "2026-04-08T06:29:06+02:00",
          "tree_id": "b7deee6ee6e759cdcbb0e20be2ddd124f918230d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/31aaf2ea1f24f481ea8307c07ea37d6d7d189688"
        },
        "date": 1775623084607,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.515,
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
            "value": 0.065,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.132,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.801,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.595,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.298,
            "range": "± 0.083",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.348,
            "range": "± 0.003",
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
            "value": 0.017,
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
            "value": 0.087,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
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
            "value": 0.182,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.942,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.4,
            "range": "± 0.041",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.946,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.133,
            "range": "± 0.372",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.539,
            "range": "± 0.007",
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
          "id": "a3a3e2031a4743b3345a934d68e47fa4b8c3dc0e",
          "message": "Release 0.7.0",
          "timestamp": "2026-04-08T06:31:08+02:00",
          "tree_id": "f34ecaa657a59e086191ed00463d789fedd1e063",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a3a3e2031a4743b3345a934d68e47fa4b8c3dc0e"
        },
        "date": 1775623247639,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.52,
            "range": "± 0.051",
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
            "value": 0.065,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.766,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.512,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.408,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.344,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.198,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.016,
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
            "value": 0.182,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.943,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.444,
            "range": "± 0.069",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.953,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.944,
            "range": "± 0.31",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.533,
            "range": "± 0.007",
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
          "id": "85277cd5f989e075580515b84054eebbfc7cd10c",
          "message": "Fix compleatino inside calls",
          "timestamp": "2026-04-08T15:32:17+02:00",
          "tree_id": "7b92a40ed10f39191fd0a2556f6fb9e73997836d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/85277cd5f989e075580515b84054eebbfc7cd10c"
        },
        "date": 1775655684635,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.63,
            "range": "± 0.166",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.064,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.13,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.264,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.17,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.822,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.643,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.043,
            "range": "± 0.001",
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
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.79,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.321,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.016,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
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
            "value": 0.175,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.941,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.682,
            "range": "± 0.186",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.955,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0",
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
            "value": 59.11,
            "range": "± 0.234",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.539,
            "range": "± 0.009",
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
          "id": "278364169302846f581b58608b8e31ee1df24e70",
          "message": "Refactor throws analysis to use PhpType for exception types",
          "timestamp": "2026-04-08T16:50:25+02:00",
          "tree_id": "02bfd84b35487d529f05b412301dad66f19d89a6",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/278364169302846f581b58608b8e31ee1df24e70"
        },
        "date": 1775661077541,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.596,
            "range": "± 0.028",
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
            "value": 0.062,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.128,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.262,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.172,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.831,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.646,
            "range": "± 0.035",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.043,
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
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.89,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.311,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.193,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "variable_completion/short",
            "value": 0.012,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "variable_completion/long",
            "value": 0.087,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.015,
            "range": "± 0.001",
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
            "value": 0.174,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.936,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.452,
            "range": "± 0.092",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.95,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
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
            "value": 62.658,
            "range": "± 0.279",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.543,
            "range": "± 0.035",
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
          "id": "3b15e5c20e317e21c373d86542c37f5a62a47574",
          "message": "Support nested array shape assignments in variable resolution",
          "timestamp": "2026-04-08T17:17:28+02:00",
          "tree_id": "88acb382247184dcc031febf2bed946dd0be8e85",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/3b15e5c20e317e21c373d86542c37f5a62a47574"
        },
        "date": 1775662061033,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.506,
            "range": "± 0.011",
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
            "value": 0.064,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.132,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.765,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.527,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.521,
            "range": "± 0.13",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.357,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.187,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.181,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.946,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.38,
            "range": "± 0.037",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.948,
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
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 58.573,
            "range": "± 0.499",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.525,
            "range": "± 0.004",
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
          "id": "be328ad3348750d779e1c1cf45d0a37f04c93e8f",
          "message": "Split large files",
          "timestamp": "2026-04-08T23:23:24+02:00",
          "tree_id": "ef622210ab49f5bce5f3341ee151d0e7a4158ce4",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/be328ad3348750d779e1c1cf45d0a37f04c93e8f"
        },
        "date": 1775683937256,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.486,
            "range": "± 0.064",
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.13,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.768,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.526,
            "range": "± 0.061",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.314,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.355,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.195,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.179,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.933,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.349,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.951,
            "range": "± 0.013",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.165,
            "range": "± 3.555",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.533,
            "range": "± 0.011",
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
          "id": "5774950dc656e9bb3b0ce9be26e8b0565d0cd0a3",
          "message": "Add invalid class-like kind diagnostics and fix bulk unused import\naction in braced namespaces",
          "timestamp": "2026-04-09T00:15:52+02:00",
          "tree_id": "b65a0c36454efc005933bb8ce5e3a51f1248c0ec",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5774950dc656e9bb3b0ce9be26e8b0565d0cd0a3"
        },
        "date": 1775687082865,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.502,
            "range": "± 0.017",
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
            "value": 0.064,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.132,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.774,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.544,
            "range": "± 0.102",
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
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.48,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.347,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.2,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.181,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.94,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.378,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.944,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.311,
            "range": "± 0.456",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.526,
            "range": "± 0.006",
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
          "id": "775146a65e7c0b59256b0078b2dae597593aad56",
          "message": "Add linked editing ranges for variables within definition regions",
          "timestamp": "2026-04-09T00:58:38+02:00",
          "tree_id": "1b04e7e7e7eaaa17dda7bd09444c42e3564024e0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/775146a65e7c0b59256b0078b2dae597593aad56"
        },
        "date": 1775689678750,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.458,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.062,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.124,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.248,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.152,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.744,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.501,
            "range": "± 0.038",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.042,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.538,
            "range": "± 0.046",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.318,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.082,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.169,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.908,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.277,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.926,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 57.919,
            "range": "± 0.798",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.514,
            "range": "± 0.015",
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
          "id": "bba3cc3dfc480e4446723512f2b846af4c14f03c",
          "message": "Add machine-readable CLI output formats for analyze and fix",
          "timestamp": "2026-04-09T03:42:40+02:00",
          "tree_id": "fb7ef75e3a7aec4f942dc98e03318c45de2424fa",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/bba3cc3dfc480e4446723512f2b846af4c14f03c"
        },
        "date": 1775699503129,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.517,
            "range": "± 0.038",
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
            "value": 0.069,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.135,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.277,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.163,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.774,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.539,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.419,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.346,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.199,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.18,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.932,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.379,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.952,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 60.3,
            "range": "± 0.859",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.521,
            "range": "± 0.021",
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
          "id": "092c292f5631655c8216d5ed8f8eec5e6b78a88e",
          "message": "Add PHPCS diagnostic proxy and update docs",
          "timestamp": "2026-04-09T04:14:43+02:00",
          "tree_id": "a9a8b0919953d0f15c766ad520c5f29b20e53b38",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/092c292f5631655c8216d5ed8f8eec5e6b78a88e"
        },
        "date": 1775701431407,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.579,
            "range": "± 0.019",
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
            "value": 0.062,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.128,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.262,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.814,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.62,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.042,
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
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.945,
            "range": "± 0.159",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.331,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.18,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.016,
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
            "value": 0.084,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.015,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.174,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.929,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.473,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.947,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.031,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 61.75,
            "range": "± 0.204",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.538,
            "range": "± 0.008",
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
          "id": "a1beb6d33f39b356250b3b847229dc343a12ce73",
          "message": "Add bulk \"Import all missing classes\" code action",
          "timestamp": "2026-04-09T04:38:22+02:00",
          "tree_id": "66fe45d8c178853fea9fa402d434398818aa4336",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a1beb6d33f39b356250b3b847229dc343a12ce73"
        },
        "date": 1775702850575,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.571,
            "range": "± 0.088",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.061,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.122,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.247,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.15,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.736,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.475,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.042,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.461,
            "range": "± 0.037",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.316,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.191,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.082,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.165,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.892,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.277,
            "range": "± 0.095",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.911,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 58.126,
            "range": "± 0.619",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.496,
            "range": "± 0.007",
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
          "id": "1e0d4372ccc820a4435aa62326b3cb7809755b4d",
          "message": "Update inlay hints demo to use email parameter in createUser call",
          "timestamp": "2026-04-09T05:09:24+02:00",
          "tree_id": "fe57b2ff55a8995bb0c37a0141fc8f3ed54daae6",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/1e0d4372ccc820a4435aa62326b3cb7809755b4d"
        },
        "date": 1775704704494,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.551,
            "range": "± 0.008",
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
            "value": 0.065,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.131,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.27,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.165,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.783,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.556,
            "range": "± 0.028",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.048,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.015,
            "range": "± 0.001",
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.418,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.347,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.202,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
            "range": "± 0.001",
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.183,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.951,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.499,
            "range": "± 0.08",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.958,
            "range": "± 0.004",
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
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.626,
            "range": "± 0.582",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.008",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sora.desousa@infomaniak.com",
            "name": "HeySora",
            "username": "HeySora"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "50e46d215b200b8b1bc1852a50065f86a78230ae",
          "message": "docs: Update Sublime Text setup instructions",
          "timestamp": "2026-04-09T16:23:17+02:00",
          "tree_id": "9fc200808929754315a9548dea2bfec209d4ad70",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/50e46d215b200b8b1bc1852a50065f86a78230ae"
        },
        "date": 1775745126046,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.465,
            "range": "± 0.057",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.062,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.122,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.247,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.15,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.737,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.474,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.041,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.468,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.315,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.082,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.167,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.909,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.395,
            "range": "± 0.096",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.921,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.03,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.029,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 58.042,
            "range": "± 0.179",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.503,
            "range": "± 0.008",
            "unit": "ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "lucasalvcoutinho@gmail.com",
            "name": "Lucas Coutinho",
            "username": "lucasacoutinho"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "e567582eb0bf8695aae84c61b878e94f198edc02",
          "message": "feat: infer property types from constructor  assignments",
          "timestamp": "2026-04-10T22:09:21+02:00",
          "tree_id": "2f78e4785335d2bbc1c2804abd6d5f403d94c8a6",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/e567582eb0bf8695aae84c61b878e94f198edc02"
        },
        "date": 1775852291523,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.544,
            "range": "± 0.037",
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
            "value": 0.066,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.271,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.793,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.577,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.048,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
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
            "value": 3.329,
            "range": "± 0.036",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.346,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.018,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.182,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.946,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.592,
            "range": "± 0.121",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.956,
            "range": "± 0.006",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.278,
            "range": "± 0.346",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.544,
            "range": "± 0.007",
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
          "id": "f0a41f07cfa7baab800d97487137e524066a84bb",
          "message": "Refactor argument splitting for template substitution",
          "timestamp": "2026-04-11T00:02:47+02:00",
          "tree_id": "60f3b847d96749feb87e3d3b08de26059f48d0a7",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/f0a41f07cfa7baab800d97487137e524066a84bb"
        },
        "date": 1775859107990,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.551,
            "range": "± 0.012",
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
            "value": 0.065,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.272,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.162,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.77,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.513,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.046,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
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
            "value": 3.527,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.357,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.086,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.933,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.379,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.948,
            "range": "± 0.013",
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
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.65,
            "range": "± 0.233",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.004",
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
          "id": "58b4b65d3ff4e72faeb21c6a40c389192bd5fe6f",
          "message": "Fix template substitution for nested class-string<T> arguments",
          "timestamp": "2026-04-11T00:12:44+02:00",
          "tree_id": "433bcb835d143dbde7a2dcba53e5f0c15de60828",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/58b4b65d3ff4e72faeb21c6a40c389192bd5fe6f"
        },
        "date": 1775859699444,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.532,
            "range": "± 0.035",
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
            "value": 0.066,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.133,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.275,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.161,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.759,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.507,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
            "range": "± 0.001",
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
            "value": 3.459,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.354,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.185,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.087,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.181,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.938,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.392,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.947,
            "range": "± 0.005",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 62.029,
            "range": "± 0.317",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.53,
            "range": "± 0.008",
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
          "id": "bdb2ceda312862b86f19c41c76ad9b5eb9bb0e9c",
          "message": "Fix property type narrowing after assignment to $this->prop",
          "timestamp": "2026-04-12T21:53:12+02:00",
          "tree_id": "f7b73464ae39455e5f186d8d9b1c0f50197ddfcb",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/bdb2ceda312862b86f19c41c76ad9b5eb9bb0e9c"
        },
        "date": 1776024182939,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.573,
            "range": "± 0.062",
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
            "value": 0.062,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.127,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.254,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.816,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.615,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.042,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
            "range": "± 0.001",
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
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.658,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.323,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.18,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.016,
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
            "value": 0.083,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.015,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
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
            "value": 0.934,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.456,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.944,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 44.095,
            "range": "± 0.227",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.464,
            "range": "± 0.013",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "aebbf8fa780b2223f6721bed12d8ad62daf4be44",
          "message": "Update README.md",
          "timestamp": "2026-04-12T21:56:58+02:00",
          "tree_id": "1a8bfaac7e70869bda46f08f0d835e499afbad58",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/aebbf8fa780b2223f6721bed12d8ad62daf4be44"
        },
        "date": 1776024367023,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.506,
            "range": "± 0.035",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.063,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.125,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.244,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.156,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.765,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.535,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.044,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_with_narrowing",
            "value": 0.014,
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
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.392,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.319,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.083,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.928,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.41,
            "range": "± 0.076",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.939,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 41.961,
            "range": "± 0.219",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.432,
            "range": "± 0.006",
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
          "id": "5c861bbcd87a94f5ec0c94cfbe6d775be78f6495",
          "message": "Rename D12 to \"Mago diagnostic proxy\" and update integration details",
          "timestamp": "2026-04-12T22:52:09+02:00",
          "tree_id": "a20f104223848819b109f359cee46160b04afea8",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5c861bbcd87a94f5ec0c94cfbe6d775be78f6495"
        },
        "date": 1776027671942,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.516,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.069,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.134,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.266,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.164,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.781,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.552,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.047,
            "range": "± 0.001",
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
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.303,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.358,
            "range": "± 0.006",
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
            "value": 0.017,
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.183,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.945,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.401,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.957,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.15,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.149,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 47.221,
            "range": "± 0.156",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.464,
            "range": "± 0.008",
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
          "id": "b9679a91bdf5839fca32664ab00026109bf9f8bd",
          "message": "Fix CI",
          "timestamp": "2026-04-12T23:24:13+02:00",
          "tree_id": "664459e938d92e79d3390c2c362f0c8aca791aac",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b9679a91bdf5839fca32664ab00026109bf9f8bd"
        },
        "date": 1776029597207,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.52,
            "range": "± 0.015",
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
            "value": 0.068,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.134,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.269,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.167,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.785,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.558,
            "range": "± 0.063",
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
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.023,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.372,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.361,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.197,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.017,
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.016,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.011,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.179,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.935,
            "range": "± 0.008",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 4.368,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.948,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.15,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.148,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 47.435,
            "range": "± 0.218",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.466,
            "range": "± 0.004",
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
          "id": "3688ea5971aad042de7c397cc692fb4381aa8857",
          "message": "Rewrite variable resolver as a single forward pass",
          "timestamp": "2026-04-20T10:15:26+02:00",
          "tree_id": "50f5fe844917cffee718230d0ffc8d84b7587ffc",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/3688ea5971aad042de7c397cc692fb4381aa8857"
        },
        "date": 1776673458561,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.527,
            "range": "± 0.034",
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
            "value": 0.085,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.151,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.289,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.176,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.79,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.57,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.059,
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
            "value": 0.022,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.452,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.413,
            "range": "± 0.038",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.203,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.028,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.039,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.033,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.182,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.005,
            "range": "± 0.025",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.336,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.017,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 40.116,
            "range": "± 0.16",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 1.106,
            "range": "± 0.015",
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
          "id": "b755b53532ae4bb832e8032630b59f6c25f07210",
          "message": "Fix CI",
          "timestamp": "2026-04-20T10:39:27+02:00",
          "tree_id": "6ba9abd1b4bc2e1df121345e21a0d4feaf22847e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b755b53532ae4bb832e8032630b59f6c25f07210"
        },
        "date": 1776674815298,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.513,
            "range": "± 0.01",
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.153,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.278,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.176,
            "range": "± 0.001",
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
            "value": 1.551,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.058,
            "range": "± 0.001",
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
            "value": 0.022,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.668,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.417,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.194,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.029,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.04,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.034,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.183,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.999,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.296,
            "range": "± 0.205",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.013,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 40.365,
            "range": "± 0.404",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 1.112,
            "range": "± 0.011",
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
          "id": "d00ac65ba38d4677d42af73e1f291e1b899d53b1",
          "message": "Eliminate ClassInfo cloning in completion pipeline",
          "timestamp": "2026-04-20T16:41:08+02:00",
          "tree_id": "5999b981a9980c5b6285dbe73c2d02383b85f2f7",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d00ac65ba38d4677d42af73e1f291e1b899d53b1"
        },
        "date": 1776696518965,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.61,
            "range": "± 0.01",
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
            "value": 0.078,
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
            "value": 0.284,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.175,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.821,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.624,
            "range": "± 0.056",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.049,
            "range": "± 0.001",
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.017,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.927,
            "range": "± 0.087",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.343,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.184,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.023,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.031,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.173,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.007,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.475,
            "range": "± 0.197",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.015,
            "range": "± 0.031",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 38.507,
            "range": "± 0.966",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.98,
            "range": "± 0.011",
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
          "id": "cdcd493d1c1f61d1d02758a3bde659c71a639fe7",
          "message": "Guard forward walker against exponential blowup on deep loop nesting",
          "timestamp": "2026-04-21T02:14:21+02:00",
          "tree_id": "4d184dea23400dbef210cc8578df75826691f205",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/cdcd493d1c1f61d1d02758a3bde659c71a639fe7"
        },
        "date": 1776730906875,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.585,
            "range": "± 0.02",
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
            "value": 0.076,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.284,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.173,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.807,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.601,
            "range": "± 0.024",
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.017,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.815,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.341,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.177,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.023,
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
            "value": 0.083,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.173,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.005,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.532,
            "range": "± 0.119",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.021,
            "range": "± 0.055",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 38.044,
            "range": "± 0.308",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.98,
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
          "id": "224ec967f5a4473d5ce74376686b798b7e20ec08",
          "message": "Tighten loop-depth guard in forward walker to prevent hangs",
          "timestamp": "2026-04-21T02:57:06+02:00",
          "tree_id": "a13d5bbc15299c60d89c42f66d3ea0e1bb25fefb",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/224ec967f5a4473d5ce74376686b798b7e20ec08"
        },
        "date": 1776733463951,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.526,
            "range": "± 0.007",
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
            "value": 0.079,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.292,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.171,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.797,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.566,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.053,
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
            "value": 0.01,
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
            "value": 3.38,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.371,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.197,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.032,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.184,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.02,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.572,
            "range": "± 0.089",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.027,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 39.43,
            "range": "± 0.72",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.99,
            "range": "± 0.007",
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
          "id": "6e735b7d4058c19ace6aa8ec62af223f2d2e7161",
          "message": "Update roadmap",
          "timestamp": "2026-04-21T03:57:20+02:00",
          "tree_id": "759b293fc64594346a06934e94e12ee7b8cf2b1f",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6e735b7d4058c19ace6aa8ec62af223f2d2e7161"
        },
        "date": 1776737083018,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.602,
            "range": "± 0.033",
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
            "value": 0.077,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.144,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.286,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.176,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.822,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.627,
            "range": "± 0.017",
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.951,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.335,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.185,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.085,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.031,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.026,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.176,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.015,
            "range": "± 0.011",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.521,
            "range": "± 0.051",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.031,
            "range": "± 0.02",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 38.68,
            "range": "± 0.312",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.977,
            "range": "± 0.009",
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
          "id": "39e28efd0ab2247954091633066e650c1e1941e7",
          "message": "Activate chain resolution cache for all LSP handlers",
          "timestamp": "2026-04-21T05:07:37+02:00",
          "tree_id": "d671c218e4215896ac9284e84163940fc0e00f45",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/39e28efd0ab2247954091633066e650c1e1941e7"
        },
        "date": 1776741287052,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.53,
            "range": "± 0.06",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.081,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.15,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.293,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.174,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.798,
            "range": "± 0.023",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.564,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.053,
            "range": "± 0.001",
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
            "value": 0.02,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.296,
            "range": "± 0.091",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.364,
            "range": "± 0.009",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.192,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.025,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.033,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.184,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.016,
            "range": "± 0.052",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.348,
            "range": "± 0.901",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.032,
            "range": "± 0.059",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 37.301,
            "range": "± 0.84",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.997,
            "range": "± 0.042",
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
          "id": "645023b0741cf2c9cad0ce2615323ac72c42cb88",
          "message": "Remove IN_ARRAY_KEY_ASSIGN re-entry guard from forward walker",
          "timestamp": "2026-04-21T06:24:42+02:00",
          "tree_id": "79068c50eb671b732391102f2cd36f4fa7f7383e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/645023b0741cf2c9cad0ce2615323ac72c42cb88"
        },
        "date": 1776745926733,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.59,
            "range": "± 0.017",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.079,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.146,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.292,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.183,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.856,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.691,
            "range": "± 0.015",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.05,
            "range": "± 0.001",
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
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.563,
            "range": "± 0.026",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.337,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.184,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.087,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.03,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.174,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.998,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.431,
            "range": "± 0.082",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.014,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 38.602,
            "range": "± 0.629",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.978,
            "range": "± 0.015",
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
          "id": "2d446dfd14a6d7af501dc0b559581a3ac6e297b9",
          "message": "Replace two-pass loop strategy with assignment-depth-bounded iteration",
          "timestamp": "2026-04-21T07:45:24+02:00",
          "tree_id": "a0205772780f5ced78ae35493350258c826c22e7",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/2d446dfd14a6d7af501dc0b559581a3ac6e297b9"
        },
        "date": 1776750762796,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.516,
            "range": "± 0.01",
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
            "value": 0.08,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.147,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.288,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.174,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.791,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.587,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.053,
            "range": "± 0.001",
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
            "value": 3.371,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.359,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.195,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.025,
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
            "value": 0.087,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.032,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.028,
            "range": "± 0.001",
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
            "value": 0.998,
            "range": "± 0.06",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.328,
            "range": "± 0.036",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.001,
            "range": "± 0.014",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 37.75,
            "range": "± 0.178",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 1.005,
            "range": "± 0.013",
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
          "id": "b6455ce8bbdea2ea5b752fd88cf050379659e84c",
          "message": "Fix infinite recursion",
          "timestamp": "2026-04-21T08:23:18+02:00",
          "tree_id": "412c7a5c8db869de0002f10264ab84d9d6c9ec65",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b6455ce8bbdea2ea5b752fd88cf050379659e84c"
        },
        "date": 1776753033859,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.506,
            "range": "± 0.077",
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
            "value": 0.08,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.149,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.294,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.17,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.768,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.521,
            "range": "± 0.067",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.053,
            "range": "± 0.001",
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.35,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.376,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.197,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.086,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.033,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.028,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.18,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.997,
            "range": "± 0.057",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.274,
            "range": "± 0.053",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.015,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.025,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 37.343,
            "range": "± 0.241",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.992,
            "range": "± 0.007",
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
          "id": "0450eb79c50896cca913ada2b866812ec5c45fbb",
          "message": "Update roadmap",
          "timestamp": "2026-04-21T09:37:18+02:00",
          "tree_id": "d4b81c11991d23bf464ff75d133af8c92d095d5c",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/0450eb79c50896cca913ada2b866812ec5c45fbb"
        },
        "date": 1776757472810,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.535,
            "range": "± 0.023",
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
            "value": 0.079,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.148,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.295,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.768,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.527,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.055,
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
            "value": 0.01,
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
            "value": 3.2,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.371,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.193,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.025,
            "range": "± 0.001",
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
            "value": 0.087,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.033,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.183,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.006,
            "range": "± 0.022",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.31,
            "range": "± 0.062",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.015,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 38.544,
            "range": "± 0.231",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 1.002,
            "range": "± 0.018",
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
          "id": "a318e41436bf142db321a61bc4ea0851fb002d7f",
          "message": "Update roadmap",
          "timestamp": "2026-04-21T18:59:28+02:00",
          "tree_id": "cf3134640dacc24ede3d94c9e6dd3481ba178c01",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a318e41436bf142db321a61bc4ea0851fb002d7f"
        },
        "date": 1776791216829,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.454,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.077,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.141,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.273,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.159,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.756,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.504,
            "range": "± 0.03",
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
            "value": 0.014,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_5_method_chain",
            "value": 0.01,
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
            "value": 3.341,
            "range": "± 0.024",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.332,
            "range": "± 0.006",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.19,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.081,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.031,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.167,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 0.983,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.205,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 0.999,
            "range": "± 0.029",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.025,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.023,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 36.368,
            "range": "± 0.083",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.928,
            "range": "± 0.008",
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
          "id": "ba6f94b984d45a7f92293a4f28c96e5beeb57fec",
          "message": "Emit closure inlay hints with call-site argument text",
          "timestamp": "2026-04-22T01:53:06+02:00",
          "tree_id": "ab1658bdbeaff3c3fc8ba0ca6c725db5cbc0a9f5",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/ba6f94b984d45a7f92293a4f28c96e5beeb57fec"
        },
        "date": 1776816029238,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.61,
            "range": "± 0.032",
            "unit": "ms"
          },
          {
            "name": "completion_simple_class",
            "value": 0.007,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 0.077,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.142,
            "range": "± 0.003",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.285,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.177,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.826,
            "range": "± 0.005",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.632,
            "range": "± 0.04",
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
            "value": 0.017,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.997,
            "range": "± 0.021",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.335,
            "range": "± 0.007",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.185,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.088,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.031,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.175,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.01,
            "range": "± 0.016",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.512,
            "range": "± 0.019",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.025,
            "range": "± 0.036",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.028,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.026,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 38.489,
            "range": "± 0.347",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.983,
            "range": "± 0.01",
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
          "id": "fc102d162b3d91aaffd8b73da8d39414cf24a7d2",
          "message": "Fix duplicate diagnostics in pull-diagnostic editors and closure/callable type error",
          "timestamp": "2026-04-22T03:01:15+02:00",
          "tree_id": "ff482ae414d52aabb1db25f6c2102707f455f796",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/fc102d162b3d91aaffd8b73da8d39414cf24a7d2"
        },
        "date": 1776820145207,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cold_start_completion",
            "value": 2.535,
            "range": "± 0.071",
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
            "value": 0.078,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 0.147,
            "range": "± 0.004",
            "unit": "ms"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 0.287,
            "range": "± 0.002",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 0.168,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 0.767,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 1.524,
            "range": "± 0.01",
            "unit": "ms"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 0.053,
            "range": "± 0.001",
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
            "value": 0.01,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 0.018,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "completion_carbon_class",
            "value": 3.45,
            "range": "± 0.013",
            "unit": "ms"
          },
          {
            "name": "completion_yii_deep_hierarchy",
            "value": 0.359,
            "range": "± 0.012",
            "unit": "ms"
          },
          {
            "name": "completion_large_file",
            "value": 0.185,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "completion_short_file",
            "value": 0.024,
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
            "value": 0.085,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "hover_method_call",
            "value": 0.032,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "goto_definition_method",
            "value": 0.027,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 0.181,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 1.012,
            "range": "± 0.027",
            "unit": "ms"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 5.347,
            "range": "± 0.03",
            "unit": "ms"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1.012,
            "range": "± 0.018",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 0.026,
            "range": "± 0.001",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 0.024,
            "range": "± 0",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 37.329,
            "range": "± 0.658",
            "unit": "ms"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 0.984,
            "range": "± 0.019",
            "unit": "ms"
          }
        ]
      }
    ]
  }
}