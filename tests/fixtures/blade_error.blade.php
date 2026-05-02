@extends('layouts.admin')
@section('header')
    @include('partials.header')
@stop
@php ($field_blade = 'admin.page._preview_field')

@section('content')
    <div class="panel panel-default min-height">
        <div class="panel-heading col-md-12">
            <h3>Preview Import</h3>
            <div class="col-md-4">
                <span class="text-original">Normal</span> is original data<br>
                If changed, <span class="text-modified">Bold</span> is modified data
                <div class="h4">
                    <span class="text-success">Total {{ count($import_rows) }} records</span>
                    @if ($row_errors->isNotEmpty())
                        <span class="text-danger">Found {{ $row_errors->count() }} errors</span>
                    @endif
                    @if ($row_warnings->isNotEmpty())
                        <span class="text-warning">Found {{ $row_warnings->count() }} warnings</span>
                    @endif
                </div>
            </div>
        </div>
    </div>
@stop