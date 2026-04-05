window.BENCHMARK_DATA = {
  "lastUpdate": 1775351725718,
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
      }
    ]
  }
}