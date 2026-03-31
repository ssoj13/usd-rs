// Browser loader for Draco WASM decoder/encoder module.
// Use as an ES module: `import DracoDecoderModule from './draco_decoder.js'`.

export async function DracoDecoderModule(options = {}) {
  const wasm = await import('./draco_js.js');
  const wasmUrl = resolveWasmUrl(options, 'draco_js_bg.wasm');
  const wasmBinary = options.wasmBinary || (await fetch(wasmUrl).then((r) => r.arrayBuffer()));
  await wasm.default(wasmBinary);
  return buildModule(wasm, options);
}

export default DracoDecoderModule;

function resolveWasmUrl(options, fileName) {
  if (typeof options.locateFile === 'function') {
    return options.locateFile(fileName, new URL('.', import.meta.url).toString());
  }
  if (typeof options.wasmUrl === 'string') {
    return options.wasmUrl;
  }
  return new URL(fileName, import.meta.url).toString();
}

function normalizeByteArray(data) {
  if (data instanceof Uint8Array) {
    return data;
  }
  if (data instanceof Int8Array) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  if (data instanceof ArrayBuffer) {
    return new Uint8Array(data);
  }
  return data;
}

function buildModule(wasm, options = {}) {
  const module = Object.assign({}, wasm);
  module.destroy = (obj) => {
    if (obj && typeof obj.free === 'function') {
      obj.free();
    }
  };
  module.isVersionSupported = isVersionSupported;

  if (module.Decoder && module.Decoder.prototype) {
    const proto = module.Decoder.prototype;
    const getEncodedGeometryTypeDeprecated = proto.GetEncodedGeometryType_Deprecated;
    proto.GetEncodedGeometryType = function (array) {
      if (array && array instanceof module.DecoderBuffer) {
        return getEncodedGeometryTypeDeprecated.call(this, array);
      }
      if (!array || array.byteLength < 8) {
        return module.INVALID_GEOMETRY_TYPE;
      }
      switch (array[7]) {
        case 0:
          return module.POINT_CLOUD;
        case 1:
          return module.TRIANGULAR_MESH;
        default:
          return module.INVALID_GEOMETRY_TYPE;
      }
    };

    wrapMeshDispatch(module, proto, 'GetAttributeId');
    wrapMeshDispatch(module, proto, 'GetAttributeIdByName');
    wrapMeshDispatch(module, proto, 'GetAttributeIdByMetadataEntry');
    wrapMeshDispatch(module, proto, 'GetAttribute');
    wrapMeshDispatch(module, proto, 'GetAttributeByUniqueId');
    wrapMeshDispatch(module, proto, 'GetMetadata');
    wrapMeshDispatch(module, proto, 'GetAttributeMetadata');
    wrapMeshDispatch(module, proto, 'GetAttributeFloatForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeIntForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeInt8ForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeUInt8ForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeInt16ForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeUInt16ForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeInt32ForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeUInt32ForAllPoints');
    wrapMeshDispatch(module, proto, 'GetAttributeDataArrayForAllPoints');

    const decodeArrayToMesh = proto.DecodeArrayToMesh;
    proto.DecodeArrayToMesh = function (data, dataSize, outMesh) {
      return decodeArrayToMesh.call(this, normalizeByteArray(data), dataSize, outMesh);
    };

    const decodeArrayToPointCloud = proto.DecodeArrayToPointCloud;
    proto.DecodeArrayToPointCloud = function (data, dataSize, outPointCloud) {
      return decodeArrayToPointCloud.call(this, normalizeByteArray(data), dataSize, outPointCloud);
    };
  }

  if (module.DecoderBuffer && module.DecoderBuffer.prototype) {
    const init = module.DecoderBuffer.prototype.Init;
    module.DecoderBuffer.prototype.Init = function (data, dataSize) {
      const normalized = normalizeByteArray(data);
      return init.call(this, normalized, dataSize ?? (normalized ? normalized.byteLength : 0));
    };
  }

  if (module.ExpertEncoder) {
    const ExpertEncoderWasm = module.ExpertEncoder;
    const ExpertEncoderWrapper = function (pc) {
      if (pc instanceof module.Mesh && typeof ExpertEncoderWasm.fromMesh === 'function') {
        return ExpertEncoderWasm.fromMesh(pc);
      }
      return new ExpertEncoderWasm(pc);
    };
    ExpertEncoderWrapper.prototype = ExpertEncoderWasm.prototype;
    Object.setPrototypeOf(ExpertEncoderWrapper, ExpertEncoderWasm);
    module.ExpertEncoder = ExpertEncoderWrapper;
  }

  if (typeof options.onRuntimeInitialized === 'function') {
    options.onRuntimeInitialized(module);
  }
  if (typeof options.onModuleParsed === 'function') {
    options.onModuleParsed(module);
  }
  if (typeof options.onModuleLoaded === 'function') {
    options.onModuleLoaded(module);
  }
  return module;
}

function wrapMeshDispatch(module, proto, baseName) {
  const meshName = `${baseName}_Mesh`;
  if (typeof proto[meshName] !== 'function') {
    return;
  }
  const baseFn = proto[baseName];
  const meshFn = proto[meshName];
  proto[baseName] = function (pcOrMesh, ...args) {
    if (pcOrMesh instanceof module.Mesh) {
      return meshFn.call(this, pcOrMesh, ...args);
    }
    return baseFn.call(this, pcOrMesh, ...args);
  };
}

function isVersionSupported(versionString) {
  if (typeof versionString !== 'string') {
    return false;
  }
  const version = versionString.split('.');
  if (version.length < 2 || version.length > 3) {
    return false;
  }
  const major = Number(version[0]);
  const minor = Number(version[1]);
  if (major === 1 && minor >= 0 && minor <= 5) {
    return true;
  }
  if (major !== 0 || minor > 10) {
    return false;
  }
  return true;
}
