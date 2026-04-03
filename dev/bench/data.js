window.BENCHMARK_DATA = {
  "lastUpdate": 1775256062735,
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
      }
    ]
  }
}